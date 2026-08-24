use nexus_engine::{
    DataMessage, IceCandidate, IceConnectionState, MediaKind, MediaStream, MediaStreamTrack,
    MediaTrackConstraints, PeerConnection, SdpType, SessionDescription, SignalingState, TrackState,
};

fn sdp(kind: SdpType) -> SessionDescription {
    SessionDescription::parse(kind, "v=0\r\na=ice-ufrag:nexus\r\na=ice-pwd:nexus-password\r\na=fingerprint:sha-256 AA:BB\r\na=group:BUNDLE 0 1\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:0\r\na=rtpmap:111 opus/48000/2\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=mid:1\r\na=rtpmap:96 VP8/90000\r\n").unwrap()
}

fn candidate(priority: u32, address: &str) -> IceCandidate {
    IceCandidate::parse(&format!("candidate:1 1 udp {priority} {address} 5000 typ host")).unwrap()
}

#[test]
fn sdp_parser_extracts_bundle_media_and_codecs() {
    let offer = sdp(SdpType::Offer);
    assert_eq!(offer.bundle_mids, vec!["0", "1"]);
    assert_eq!(offer.media.len(), 2);
    assert_eq!(offer.media[0].codecs[0].name, "opus");
    assert_eq!(offer.media[1].kind, MediaKind::Video);
}

#[test]
fn malformed_sdp_without_security_parameters_is_rejected() {
    assert!(SessionDescription::parse(SdpType::Offer, "v=0\r\nm=audio 9 RTP/AVP 0\r\n").is_err());
}

#[test]
fn ice_candidate_parser_reads_transport_priority_and_type() {
    let parsed = IceCandidate::parse("candidate:a 1 UDP 2122260223 192.0.2.1 54400 typ host").unwrap();
    assert_eq!(parsed.priority, 2_122_260_223);
    assert_eq!(parsed.address, "192.0.2.1");
}

#[test]
fn offer_answer_and_candidates_reach_connected_control_state() {
    let mut peer = PeerConnection::new();
    peer.set_local_description(sdp(SdpType::Offer)).unwrap();
    assert_eq!(peer.signaling_state(), SignalingState::HaveLocalOffer);
    peer.set_remote_description(sdp(SdpType::Answer)).unwrap();
    peer.add_local_candidate(candidate(100, "10.0.0.1")).unwrap();
    peer.add_remote_candidate(candidate(200, "10.0.0.2")).unwrap();
    assert_eq!(peer.ice_state(), IceConnectionState::Connected);
    assert_eq!(peer.selected_pair().unwrap().remote.address, "10.0.0.2");
}

#[test]
fn media_stream_rejects_duplicate_tracks_and_can_stop_capture() {
    let track = MediaStreamTrack { id: "camera".into(), kind: MediaKind::Video, label: "Front camera".into(), enabled: true, muted: false, state: TrackState::Live, constraints: MediaTrackConstraints { width: Some(1280), height: Some(720), frame_rate: Some(30), sample_rate: None, echo_cancellation: None } };
    let mut stream = MediaStream::new("local");
    stream.add_track(track.clone()).unwrap();
    assert!(stream.add_track(track).is_err());
    let mut removed = stream.remove_track("camera").unwrap(); removed.stop();
    assert_eq!(removed.state, TrackState::Ended);
}

#[test]
fn data_channel_applies_backpressure_and_releases_buffered_bytes() {
    let mut peer = PeerConnection::new();
    let index = peer.create_data_channel("chat", true, 5).unwrap();
    let channel = peer.data_channel_mut(index).unwrap(); channel.open();
    channel.send(DataMessage::Text("hello".into())).unwrap();
    assert!(channel.send(DataMessage::Text("!".into())).is_err());
    assert_eq!(channel.buffered_bytes(), 5);
    assert_eq!(channel.take_outgoing(), Some(DataMessage::Text("hello".into())));
    assert_eq!(channel.buffered_bytes(), 0);
}

#[test]
fn closing_peer_connection_closes_channels_and_rejects_changes() {
    let mut peer = PeerConnection::new();
    let index = peer.create_data_channel("events", false, 64).unwrap();
    peer.close();
    assert_eq!(peer.signaling_state(), SignalingState::Closed);
    assert!(peer.add_local_candidate(candidate(1, "127.0.0.1")).is_err());
    assert_eq!(peer.data_channel_mut(index).unwrap().state, nexus_engine::DataChannelState::Closed);
}
