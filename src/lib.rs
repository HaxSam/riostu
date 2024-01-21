mod error;

use std::io::SeekFrom;
use std::io::{Read, Seek};
use std::pin::Pin;
use std::task::{Context, Poll};

use isahc::prelude::*;
use isahc::{AsyncBody, Request, ResponseFuture};
use smol::future::FutureExt;
use smol::io::{AsyncRead, AsyncSeek};

use error::RemoteIoError;

pub struct RemoteIO {
    url: Box<str>,
    content_size: u64,
    pos: u64,
    status: Status,
}

#[derive(Default)]
enum Status {
    #[default]
    None,
    Request(ResponseFuture<'static>),
    Response(AsyncBody),
}

impl From<ResponseFuture<'static>> for Status {
    fn from(value: ResponseFuture<'static>) -> Self {
        Self::Request(value)
    }
}

impl From<AsyncBody> for Status {
    fn from(value: AsyncBody) -> Self {
        Self::Response(value)
    }
}

impl From<isahc::Response<AsyncBody>> for Status {
    fn from(value: isahc::Response<AsyncBody>) -> Self {
        Self::Response(value.into_body())
    }
}

impl RemoteIO {
    pub fn new(url: &str) -> Result<Self, RemoteIoError> {
        let head = isahc::head(url)?;
        let _ = head
            .headers()
            .get("Accept-Ranges")
            .ok_or(RemoteIoError::NotSeekable)?;
        let content_size: u64 = head
            .headers()
            .get("Content-Length")
            .ok_or(RemoteIoError::NoContentSize)?
            .to_str()
            .expect("Couldn't convert content-size to str")
            .parse()
            .expect("Couldn't parse content-size to u64");

        Ok(RemoteIO {
            url: url.into(),
            content_size,
            pos: 0,
            status: Status::None,
        })
    }

    pub fn wait(self) -> impl Read + Seek {
        Self::block(self)
    }

    pub fn block(blank: Self) -> impl Read + Seek {
        smol::io::BlockOn::new(blank)
    }

    fn create_request(&self, size: u64) -> Request<&str> {
        isahc::Request::get(self.url.as_ref())
            .header("Range", format!("bytes={}-{}", self.pos, self.pos + size))
            .body("")
            .unwrap()
    }
}

impl AsyncRead for RemoteIO {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let to_consume = buf.len() as u64;

        let (status, poll) = match &mut self.status {
            Status::None => {
                let new_status: Status = self.create_request(to_consume - 1).send_async().into();
                (Some(new_status), Poll::<std::io::Result<usize>>::Pending)
            }
            Status::Request(req) => match req.poll(cx) {
                Poll::Ready(res) => (Some(res.unwrap().into()), Poll::Pending),
                Poll::Pending => (None, Poll::Pending),
            },
            Status::Response(res) => match Pin::new(res).poll_read(cx, buf) {
                Poll::Ready(size) => {
                    let size = size.unwrap();
                    self.pos += size as u64;
                    (Some(Status::None), Poll::Ready(Ok(size)))
                }
                Poll::Pending => (None, Poll::Pending),
            },
        };

        if let Some(status) = status {
            self.status = status;
            cx.waker().wake_by_ref();
        };

        poll
    }
}

impl AsyncSeek for RemoteIO {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        match pos {
            SeekFrom::Start(pos) => self.pos = pos,
            SeekFrom::End(pos) => self.pos = ((self.content_size as i64) + pos) as u64,
            SeekFrom::Current(pos) => self.pos = ((self.pos as i64) + pos) as u64,
        }

        Poll::Ready(Ok(self.pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_URL: &str =
        "https://oxygenos.oneplus.net/OnePlus8TOxygen_15.E.29_OTA_0290_all_2110091931_downgrade";

    #[test]
    fn rio_creation() {
        let rio = RemoteIO::new(TEST_URL);
        eprintln!("{}", rio.unwrap().content_size);
    }

    #[test]
    fn read_bytes() {
        use std::io::Read;

        let rio = RemoteIO::new(TEST_URL).unwrap();
        let mut buf = vec![0u8; 1024];
        let mut rio_sync = RemoteIO::block(rio);

        rio_sync.read_exact(&mut buf).unwrap();
    }

    #[test]
    fn read_zip() {
        use std::io::BufReader;
        use zip::ZipArchive;

        let rio = RemoteIO::new(TEST_URL).unwrap();
        let buf_reader = BufReader::new(rio.wait());

        let mut zip = ZipArchive::new(buf_reader).unwrap();
        for i in 0..zip.len() {
            let file = zip.by_index(i).unwrap();
            eprintln!("File Name: {}, size: {}", file.name(), file.size())
        }
    }

    #[allow(dead_code)]
    fn save_zip() {
        use smol::fs::File;
        use smol::io::{self, BufReader, BufWriter};

        smol::block_on(async {
            let rio = RemoteIO::new(TEST_URL).unwrap();
            let file = File::create("test.zip").await.unwrap();

            let rio = BufReader::new(rio);
            let file = BufWriter::new(file);

            io::copy(rio, file).await.unwrap();
        })
    }
}
