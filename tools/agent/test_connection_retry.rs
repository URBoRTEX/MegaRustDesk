#[path = "../../src/connection_retry.rs"]
mod connection_retry;
use connection_retry::{retry_before_login, unless_cancelled, RelayHandshakeTimeout};
use hbb_common::{anyhow::anyhow, tokio::{self, net::{TcpListener,TcpStream}, io::{AsyncReadExt,AsyncWriteExt}, time::{timeout,Duration}}, ResultType};
use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};

fn main() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        // Real sockets: first relay leg is silent, second returns a handshake frame.
        let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr=listener.local_addr().unwrap();
        let accepted=Arc::new(AtomicUsize::new(0));
        let count=accepted.clone();
        let server=tokio::spawn(async move {
            let (mut first,_)=listener.accept().await.unwrap();count.fetch_add(1,Ordering::SeqCst);
            let mut byte=[0];assert_eq!(first.read(&mut byte).await.unwrap(),0,"failed transport must be dropped before next attempt");
            let (mut second,_)=listener.accept().await.unwrap();count.fetch_add(1,Ordering::SeqCst);
            second.write_all(b"handshake").await.unwrap();
        });
        let mut attempts=0;
        let got=retry_before_login(|| {
            attempts+=1;
            async move {
                let mut stream=TcpStream::connect(addr).await?;
                let mut bytes=[0;9];
                timeout(Duration::from_millis(40),stream.read_exact(&mut bytes)).await
                    .map_err(|e|anyhow!(e).context(RelayHandshakeTimeout))??;
                Ok(bytes)
            }
        }).await.unwrap();
        assert_eq!(&got,b"handshake");assert_eq!(attempts,2);server.await.unwrap();
        assert_eq!(accepted.load(Ordering::SeqCst),2);
        let mut n=0;
        let exhausted:ResultType<()>=retry_before_login(||{n+=1;async{Err(anyhow!("deadline has elapsed").context(RelayHandshakeTimeout))}}).await;
        assert!(exhausted.is_err());assert_eq!(n,3);
        for reason in ["Signature mismatch in key exchange","Key mismatch","Wrong Password","Remote desktop is offline","invalid message format","Peer handshake send timed out","deadline has elapsed"] {
            let mut n=0;
            let result:ResultType<()>=retry_before_login(||{n+=1;async{Err(anyhow!(reason.to_owned()))}}).await;
            assert!(result.is_err());assert_eq!(n,1,"must not retry security/login/unclassified failures");
        }
        let mut n=0;
        let success=retry_before_login(||{n+=1;async{Ok(7)}}).await.unwrap();
        assert_eq!(success,7);assert_eq!(n,1,"no operation after successful handshake");
        let attempts=Arc::new(AtomicUsize::new(0));let count=attempts.clone();
        let result=unless_cancelled(retry_before_login(||{count.fetch_add(1,Ordering::SeqCst);async{Err::<(),_>(anyhow!("timeout").context(RelayHandshakeTimeout))}}),async {tokio::time::sleep(Duration::from_millis(10)).await}).await;
        assert!(result.is_none());assert_eq!(attempts.load(Ordering::SeqCst),1,"cancel during backoff must not reconnect");
        let result=unless_cancelled(std::future::pending::<()>(),async{}).await;
        assert!(result.is_none(),"cancel must drop an in-flight connect");
        println!("PASS: real silent relay recovery, fresh socket, bounded attempts, security/login exclusions, success once, cancellation");
    });
}
