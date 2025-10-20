use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use tokio::{sync::Mutex, task};
use webrtc::{api::{setting_engine::SettingEngine, APIBuilder, API}, data_channel::{data_channel_init::RTCDataChannelInit, RTCDataChannel}, ice_transport::{ice_candidate::RTCIceCandidate, ice_server::RTCIceServer}, peer_connection::{configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription, RTCPeerConnection}};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct ConnectInfo {
    pub sdp: RTCSessionDescription,
    pub candidates: Vec<RTCIceCandidate>,
}

#[derive(Clone)]
pub struct WebRTCConnection {
    pub api: Arc<API>,
    pub peer: Arc<RTCPeerConnection>,
    pub channel: Arc<RTCDataChannel>,
    pub tcp_channel: Arc<RTCDataChannel>,
    pub connect_info: String,
}

async fn set_peer_answer(connection: &WebRTCConnection, peer_connect_info: String) -> Result<(), Box<dyn std::error::Error>> {
    let peer_connect_info: ConnectInfo = serde_json::from_str(&String::from_utf8(base64::decode(peer_connect_info)?)?)?;

    connection.peer.set_remote_description(peer_connect_info.sdp).await?;

    for candidate in peer_connect_info.candidates {
        connection.peer.add_ice_candidate(candidate.to_json()?).await?;
    }

    Ok(())
}

impl WebRTCConnection {
    async fn send_blocking_internal(&self, buffer: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.channel.send(&Bytes::copy_from_slice(buffer)).await?;
        Ok(())
    }

    pub fn send_blocking(&self, runtime: &tokio::runtime::Runtime, buffer: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        task::block_in_place(|| {
            runtime.block_on(async {
                self.send_blocking_internal(buffer).await
            })
        })?;

        Ok(())
    }

    pub fn set_answer(&self, runtime: &tokio::runtime::Runtime, peer_connect_info: String) -> Result<(), Box<dyn std::error::Error>> {
        task::block_in_place(|| {
            runtime.block_on(async {
                set_peer_answer(&self, peer_connect_info).await
            })
        })?;

        Ok(())
    }
}

pub fn create_offerer(runtime: &tokio::runtime::Runtime) -> Result<WebRTCConnection, Box<dyn std::error::Error>> {
    task::block_in_place(|| {
        runtime.block_on(async {
            // Create API for the WebRTC connection
            let mut settings = SettingEngine::default();
            settings.set_ice_timeouts(Some(Duration::from_secs(300)), Default::default(), Default::default());

            let api = Arc::new(APIBuilder::new().with_setting_engine(settings).build());

            let config = RTCConfiguration {
                ice_servers: vec![RTCIceServer {
                    urls: vec![
                        "stun:stun.l.google.com:19302".to_owned(),
                        "stun:stun.l.google.com:5349".to_owned(),
                        "stun:stun1.l.google.com:3478".to_owned(),
                        "stun:stun1.l.google.com:5349".to_owned(),
                        "stun:stun2.l.google.com:19302".to_owned(),
                        "stun:stun2.l.google.com:5349".to_owned(),
                        "stun:stun3.l.google.com:3478".to_owned(),
                        "stun:stun3.l.google.com:5349".to_owned(),
                        "stun:stun4.l.google.com:19302".to_owned(),
                        "stun:stun4.l.google.com:5349".to_owned(),
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            };

            // Create a new RTCPeerConnection
            let peer_connection = Arc::new(api.new_peer_connection(config).await?);

            let data_channel = peer_connection.create_data_channel("audio", Some(RTCDataChannelInit { ordered: Some(false), negotiated: Some(0), max_retransmits: None, protocol: None, ..Default::default() })).await?;
            let tcp_data_channel = peer_connection.create_data_channel("tcp", Some(RTCDataChannelInit { negotiated: Some(0), ..Default::default() })).await?;

            let gathered_candidates: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(Vec::new()));

            let gc2 = gathered_candidates.clone();
            peer_connection.on_ice_candidate(Box::new(move |candidate| {
                let gc3 = gc2.clone();
                Box::pin(async move {
                    if let Some(candidate) = candidate {
                        let mut gc = gc3.lock().await;
                        gc.push(candidate);
                    }
                })
            }));

            let offer = peer_connection.create_offer(None).await?;

            peer_connection.set_local_description(offer.clone()).await?;

            // Create channel that is blocked until ICE Gathering is complete
            let mut gather_complete = peer_connection.gathering_complete_promise().await;
            let _ = gather_complete.recv().await;

            let candidates = gathered_candidates.lock().await;

            // return ConnectInfo
            let connect_info = 
                ConnectInfo {
                    sdp: offer,
                    candidates: candidates.to_vec(),
                };

            Ok (
                WebRTCConnection {
                    api,
                    peer: peer_connection,
                    channel: data_channel,
                    tcp_channel: tcp_data_channel,
                    connect_info: base64::encode(serde_json::to_string(&connect_info)?),
                }
            )
        })
    })
}

pub fn create_answerer(runtime: &tokio::runtime::Runtime, peer_connect_info: String) -> Result<WebRTCConnection, Box<dyn std::error::Error>> {
    task::block_in_place(|| {
        runtime.block_on(async {
            // Create API for the WebRTC connection
            let mut settings = SettingEngine::default();
            settings.set_ice_timeouts(Some(Duration::from_secs(300)), Default::default(), Default::default());

            let api = Arc::new(APIBuilder::new().with_setting_engine(settings).build());

            let config = RTCConfiguration {
                ice_servers: vec![RTCIceServer {
                    urls: vec![
                        "stun:stun.l.google.com:19302".to_owned(),
                        "stun:stun.l.google.com:5349".to_owned(),
                        "stun:stun1.l.google.com:3478".to_owned(),
                        "stun:stun1.l.google.com:5349".to_owned(),
                        "stun:stun2.l.google.com:19302".to_owned(),
                        "stun:stun2.l.google.com:5349".to_owned(),
                        "stun:stun3.l.google.com:3478".to_owned(),
                        "stun:stun3.l.google.com:5349".to_owned(),
                        "stun:stun4.l.google.com:19302".to_owned(),
                        "stun:stun4.l.google.com:5349".to_owned(),
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            };
            // Create a new RTCPeerConnection
            let peer_connection = Arc::new(api.new_peer_connection(config).await?);

            let data_channel = peer_connection.create_data_channel("audio", Some(RTCDataChannelInit { ordered: Some(false), negotiated: Some(0), max_retransmits: None, protocol: None, ..Default::default() })).await?;
            let tcp_data_channel = peer_connection.create_data_channel("tcp", Some(RTCDataChannelInit { negotiated: Some(0), ..Default::default() })).await?;

            let gathered_candidates: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(Vec::new()));

            let gc2 = gathered_candidates.clone();
            peer_connection.on_ice_candidate(Box::new(move |candidate| {
                let gc3 = gc2.clone();
                Box::pin(async move {
                    if let Some(candidate) = candidate {
                        let mut gc = gc3.lock().await;
                        gc.push(candidate);
                    }
                })
            }));

            let peer_connect_info: ConnectInfo = serde_json::from_str(&String::from_utf8(base64::decode(peer_connect_info)?)?)?;

            peer_connection.set_remote_description(peer_connect_info.sdp).await?;

            let answer = peer_connection.create_answer(None).await?;

            peer_connection.set_local_description(answer.clone()).await?;

            for candidate in peer_connect_info.candidates {
                peer_connection.add_ice_candidate(candidate.to_json()?).await?;
            }

            // Create channel that is blocked until ICE Gathering is complete
            let mut gather_complete = peer_connection.gathering_complete_promise().await;
            let _ = gather_complete.recv().await;

            let candidates = gathered_candidates.lock().await;

            // return ConnectInfo
            let connect_info = 
                ConnectInfo {
                    sdp: answer,
                    candidates: candidates.to_vec(),
                };

            Ok (
                WebRTCConnection {
                    api,
                    peer: peer_connection,
                    channel: data_channel,
                    tcp_channel: tcp_data_channel,
                    connect_info: base64::encode(serde_json::to_string(&connect_info)?),
                }
            )
        })
    })
}