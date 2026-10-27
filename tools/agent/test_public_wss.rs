#[path = "../../src/agent/network.rs"]
mod network;
fn main(){
 for (raw,expected) in [("rs-ny.rustdesk.com:21116","wss://rs-ny.rustdesk.com/ws/id"),("rs-ny.rustdesk.com:21115","wss://rs-ny.rustdesk.com/ws/id"),("ovh-fr2.rustdesk.com:21117","wss://ovh-fr2.rustdesk.com/ws/relay")]{
  assert_eq!(network::public_endpoint(raw,false).as_deref(),Some(expected));
  assert_eq!(network::public_endpoint(raw,true),None,"configured proxy route must not be bypassed");
 }
 for raw in ["example.invalid:21116","192.0.2.1:21117","[2001:db8::1]:21117","ovh-fr2.rustdesk.com.evil.invalid:21117","ovh-fr2.rustdesk.com:12345","wss://example.invalid/ws/relay","ws://example.invalid:21119","user%3Asecret@ovh-fr2.rustdesk.com:21117","evil.invalid/#ovh-fr2.rustdesk.com:21117","x..rustdesk.com:21117"]{
  assert_eq!(network::public_endpoint(raw,false),None,"custom endpoints must be preserved");
 }
 println!("PASS: public coordinator/online/relay use WSS, custom endpoints and proxy routes preserved");
 if std::env::args().any(|a|a=="--live") {
  std::env::set_var("RUSTDESK_AGENT_DATA",std::env::current_dir().unwrap().join(".agent-build/wss-policy-fixture"));
  *hbb_common::config::APP_NAME.write().unwrap()="MegaRustDeskNetworkFixture".into();
  hbb_common::sodiumoxide::init().unwrap();
  hbb_common::tokio::runtime::Runtime::new().unwrap().block_on(async {
    use hbb_common::{config, rendezvous_proto::{RendezvousMessage,RequestRelay},message_proto::{Message,IdPk,SignedId,PublicKey},protobuf::Message as _,sodiumoxide::crypto::{sign,box_,secretbox}};
   for _ in 0..3 {
    let uuid=hbb_common::uuid::Uuid::new_v4().to_string();
    let mut a=network::connect_tcp("ovh-fr2.rustdesk.com:21117",7000).await.unwrap();
    let mut b=network::connect_tcp("ovh-fr2.rustdesk.com:21117",7000).await.unwrap();
    assert!(matches!(a,hbb_common::Stream::WebSocket(_)));
    assert!(matches!(&a,hbb_common::Stream::WebSocket(ws) if ws.is_tls()));
    let mut m=RendezvousMessage::new();
    m.set_request_relay(RequestRelay{uuid:uuid.clone(),licence_key:config::RS_PUB_KEY.to_owned(),..Default::default()});
    a.send(&m).await.unwrap();
    let (sign_pk,sk)=sign::gen_keypair();let (pk,receiver_sk)=box_::gen_keypair();
    let signed=sign::sign(&IdPk{id:"probe0001".into(),pk:pk.0.to_vec().into(),..Default::default()}.write_to_bytes().unwrap(),&sk);
    let mut payload=Message::new();payload.set_signed_id(SignedId{id:signed.into(),..Default::default()});
    let expected=payload.write_to_bytes().unwrap();assert_eq!(expected.len(),113);
    a.send(&payload).await.unwrap();
    hbb_common::tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    m.set_request_relay(RequestRelay{id:"probe0001".into(),uuid,licence_key:config::RS_PUB_KEY.to_owned(),..Default::default()});
    b.send(&m).await.unwrap();
    assert_eq!(b.next_timeout(5000).await.unwrap().unwrap().as_ref(),expected.as_slice());
    let verified=sign::verify(&payload.signed_id().id,&sign_pk).unwrap();
    let identity=IdPk::parse_from_bytes(&verified).unwrap();assert_eq!(identity.id,"probe0001");assert_eq!(identity.pk.len(),32);
    let (viewer_pk,viewer_sk)=box_::gen_keypair();let key=secretbox::gen_key();
    let sealed=box_::seal(&key.0,&box_::Nonce([0;24]),&pk,&viewer_sk);
    let mut reply=Message::new();reply.set_public_key(PublicKey{asymmetric_value:viewer_pk.0.to_vec().into(),symmetric_value:sealed.into(),..Default::default()});
    b.send(&reply).await.unwrap();
    let raw=a.next_timeout(5000).await.unwrap().unwrap();let reply=Message::parse_from_bytes(&raw).unwrap();
    let decoded=box_::open(&reply.public_key().symmetric_value,&box_::Nonce([0;24]),&viewer_pk,&receiver_sk).unwrap();
    assert_eq!(decoded,key.0);a.set_key(secretbox::Key::from_slice(&decoded).unwrap());b.set_key(key);
    for size in [64,16384,65536] {
     let bytes=hbb_common::sodiumoxide::randombytes::randombytes(size);
     a.send_raw(bytes.clone()).await.unwrap();assert_eq!(b.next_timeout(5000).await.unwrap().unwrap().as_ref(),bytes.as_slice());
     b.send_raw(bytes.clone()).await.unwrap();assert_eq!(a.next_timeout(5000).await.unwrap().unwrap().as_ref(),bytes.as_slice());
    }
   }
   let coordinator=network::connect_tcp("rs-ny.rustdesk.com:21116",7000).await.unwrap();
   assert!(matches!(coordinator,hbb_common::Stream::WebSocket(_)));
   let listener=hbb_common::tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
   let address=listener.local_addr().unwrap();
   let peer=hbb_common::tokio::spawn(async move {listener.accept().await.unwrap()});
   let tcp=hbb_common::tokio::net::TcpStream::connect(address).await.unwrap();let accepted=peer.await.unwrap();
   let plain=hbb_common::websocket::WsFramedStream::from_tcp_stream(tcp,address).await.unwrap();
   assert!(!plain.is_tls(),"plain WebSocket must not be considered TLS-secured");drop(accepted);
   println!("PASS: native certificate-verified public WSS, verified SignedId113 + boxed peer key exchange + 163968 encrypted bidirectional bytes x3, coordinator TLS upgrade");
  });
 }
}
