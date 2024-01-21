use std::io::SeekFrom;
use std::pin::Pin;
use std::task::{Context, Poll};

use isahc::prelude::*;
use isahc::{AsyncBody, Request, ResponseFuture};
use smol::future::FutureExt;
use smol::io::{AsyncRead, AsyncSeek};

pub struct Blank {
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

impl Blank {
    pub fn new(url: &str) -> Option<Self> {
        let head = isahc::head(url).ok()?;
        let _ = head.headers().get("Accept-Ranges")?;
        let content_size: u64 = head
            .headers()
            .get("Content-Length")?
            .to_str()
            .ok()?
            .parse()
            .ok()?;

        Some(Blank {
            url: url.into(),
            content_size,
            pos: 0,
            status: Status::None,
        })
    }

    fn create_request(&self, size: u64) -> Request<&str> {
        isahc::Request::get(self.url.as_ref())
            .header("Range", format!("bytes={}-{}", self.pos, self.pos + size))
            .body("")
            .unwrap()
    }

    pub fn make_sync(blank: Self) -> impl std::io::Read + std::io::Seek {
        smol::io::BlockOn::new(blank)
    }
}

impl AsyncRead for Blank {
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

impl AsyncSeek for Blank {
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
    static T: &str =
        "https://oxygenos.oneplus.net/OnePlus8TOxygen_15.E.29_OTA_0290_all_2110091931_downgrade";

    #[test]
    fn lel() {
        let b = Blank::new(T);
        eprintln!("{}", b.unwrap().content_size);
    }

    #[test]
    fn read() {
        let b = Blank::new(T).unwrap();
        let mut buf = vec![0u8; 1024];
        let mut t = smol::io::BlockOn::new(b);
        t.read_exact(&mut buf).unwrap();
        eprintln!("{}", buf.len());
    }

    #[test]
    fn try_read_zip() {
        use zip::ZipArchive;
        let b = Blank::new(T).unwrap();
        let u = Blank::make_sync(b);
        let u = std::io::BufReader::new(u);

        let mut zip = ZipArchive::new(u).unwrap();
        for i in 0..zip.len() {
            let file = zip.by_index(i).unwrap();
            eprintln!("File Name: {}, size: {}", file.name(), file.size())
        }
    }

    #[allow(dead_code)]
    fn save_zip() {
        use smol::fs::File;
        smol::block_on(async {
            let b = Blank::new(T).unwrap();
            let file = File::create("test.zip").await.unwrap();

            let b = smol::io::BufReader::new(b);
            let file = smol::io::BufWriter::new(file);

            smol::io::copy(b, file).await.unwrap();
        })
    }
}
