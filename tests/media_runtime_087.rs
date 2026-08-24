use std::time::Duration;

use nexus_engine::{
    parse_codec_string, sniff_container, DecodedMedia, DecoderBackend, DecoderCapability,
    DecoderRegistry, EncodedPacket, MediaCodec, MediaContainer, MediaElementKind, MediaPipeline,
    MediaState, MediaTrack, PixelFormat,
};

fn registry() -> DecoderRegistry {
    let mut registry = DecoderRegistry::default();
    for codec in [MediaCodec::Av1, MediaCodec::Vp9, MediaCodec::H264, MediaCodec::Opus, MediaCodec::Aac] {
        registry.register(DecoderCapability { codec, backend: DecoderBackend::Software, hardware_accelerated: false, secure_decode: false, max_width: 3840, max_height: 2160 });
        registry.register(DecoderCapability { codec, backend: DecoderBackend::AndroidMediaCodec, hardware_accelerated: true, secure_decode: true, max_width: 7680, max_height: 4320 });
    }
    registry
}

fn video_track() -> MediaTrack {
    MediaTrack { id: 1, kind: MediaElementKind::Video, codec: MediaCodec::H264, duration: Duration::from_secs(2), width: 1920, height: 1080, sample_rate: 0, channels: 0 }
}

#[test]
fn container_and_codec_detection_covers_modern_media() {
    assert_eq!(sniff_container(b"\0\0\0\x18ftypisom"), MediaContainer::Mp4);
    assert_eq!(sniff_container(&[0x1a, 0x45, 0xdf, 0xa3]), MediaContainer::WebM);
    assert_eq!(parse_codec_string("av01.0.08M.08"), Some(MediaCodec::Av1));
    assert_eq!(parse_codec_string("avc1.640028"), Some(MediaCodec::H264));
}

#[test]
fn hardware_decoder_is_selected_when_requested() {
    let mut pipeline = MediaPipeline::new(MediaElementKind::Video);
    pipeline.load_metadata(vec![video_track()], &registry(), true).unwrap();
    pipeline.enqueue_packet(EncodedPacket { track_id: 1, pts: Duration::ZERO, dts: Duration::ZERO, duration: Duration::from_millis(33), keyframe: true, bytes: vec![1] }).unwrap();
    let (_, decoder) = pipeline.take_packet_for_decode().unwrap();
    assert_eq!(decoder.backend, DecoderBackend::AndroidMediaCodec);
}

#[test]
fn media_clock_drives_play_pause_and_seek() {
    let mut pipeline = MediaPipeline::new(MediaElementKind::Video);
    pipeline.load_metadata(vec![video_track()], &registry(), false).unwrap();
    pipeline.play(Duration::from_secs(10)).unwrap();
    assert_eq!(pipeline.snapshot(Duration::from_secs(11)).current_time, Duration::from_secs(1));
    pipeline.pause(Duration::from_secs(11));
    assert_eq!(pipeline.snapshot(Duration::from_secs(20)).current_time, Duration::from_secs(1));
    pipeline.seek(Duration::from_millis(250), Duration::from_secs(20)).unwrap();
    assert_eq!(pipeline.snapshot(Duration::from_secs(20)).current_time, Duration::from_millis(250));
}

#[test]
fn decoded_frames_are_presented_in_timestamp_order() {
    let mut pipeline = MediaPipeline::new(MediaElementKind::Video);
    pipeline.load_metadata(vec![video_track()], &registry(), false).unwrap();
    for pts in [200, 100] {
        pipeline.push_decoded(DecodedMedia::Video { track_id: 1, pts: Duration::from_millis(pts), duration: Duration::from_millis(33), width: 1, height: 1, format: PixelFormat::Rgba8, planes: vec![vec![0; 4]] }).unwrap();
    }
    pipeline.play(Duration::ZERO).unwrap();
    let ready = pipeline.take_presentable(Duration::from_millis(150));
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].pts(), Duration::from_millis(100));
}

#[test]
fn late_video_frames_are_dropped_for_av_sync() {
    let mut pipeline = MediaPipeline::new(MediaElementKind::Video);
    pipeline.load_metadata(vec![video_track()], &registry(), false).unwrap();
    pipeline.push_decoded(DecodedMedia::Video { track_id: 1, pts: Duration::ZERO, duration: Duration::from_millis(20), width: 1, height: 1, format: PixelFormat::Rgba8, planes: vec![vec![0; 4]] }).unwrap();
    pipeline.play(Duration::ZERO).unwrap();
    assert!(pipeline.take_presentable(Duration::from_millis(200)).is_empty());
    assert_eq!(pipeline.snapshot(Duration::from_millis(200)).dropped_video_frames, 1);
}

#[test]
fn end_of_stream_transitions_media_element_to_ended() {
    let mut track = video_track(); track.duration = Duration::from_millis(100);
    let mut pipeline = MediaPipeline::new(MediaElementKind::Video);
    pipeline.load_metadata(vec![track], &registry(), false).unwrap();
    pipeline.mark_end_of_stream(); pipeline.play(Duration::ZERO).unwrap();
    pipeline.take_presentable(Duration::from_millis(100));
    assert_eq!(pipeline.snapshot(Duration::from_millis(100)).state, MediaState::Ended);
}

#[test]
fn packets_for_unknown_tracks_are_rejected() {
    let mut pipeline = MediaPipeline::new(MediaElementKind::Audio);
    pipeline.load_metadata(vec![video_track()], &registry(), false).unwrap();
    let result = pipeline.enqueue_packet(EncodedPacket { track_id: 99, pts: Duration::ZERO, dts: Duration::ZERO, duration: Duration::ZERO, keyframe: true, bytes: vec![] });
    assert!(result.is_err());
}
