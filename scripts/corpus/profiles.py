"""Tool-bound recipes; encoded digests are observed per pinned toolchain."""

NATIVE_PROFILES = [
    {"id": "ani.native.mp4-cfr", "container": "mp4", "video_streams": 1, "audio_streams": 0, "b_frames": 0, "sample_aspect_ratio": "1:1"},
    {"id": "ani.native.mp4-b-frames", "container": "mp4", "video_streams": 1, "audio_streams": 0, "b_frames": 2, "sample_aspect_ratio": "1:1"},
    {"id": "ani.native.mov-sar", "container": "mov", "video_streams": 1, "audio_streams": 0, "b_frames": 0, "sample_aspect_ratio": "2:1"},
    {"id": "ani.native.mkv-dual-video", "container": "matroska", "video_streams": 2, "audio_streams": 0, "b_frames": 0, "sample_aspect_ratio": "1:1"},
    {"id": "ani.native.mkv-audio-video", "container": "matroska", "video_streams": 1, "audio_streams": 1, "b_frames": 0, "sample_aspect_ratio": "1:1"},
]
