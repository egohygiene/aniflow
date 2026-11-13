#!/bin/bash

# Check if a file was provided
if [ -z "$1" ]; then
    echo "Usage: ./check_deovr.sh <path_to_video_file>"
    exit 1
fi

VIDEO_FILE="$1"

# Extract metadata using ffprobe
METADATA=$(ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,profile,pix_fmt,width,height -of default=noprint_wrappers=1 "$VIDEO_FILE")

# Parse variables
CODEC=$(echo "$METADATA" | grep "codec_name" | cut -d= -f2)
PROFILE=$(echo "$METADATA" | grep "profile" | cut -d= -f2)
PIX_FMT=$(echo "$METADATA" | grep "pix_fmt" | cut -d= -f2)
WIDTH=$(echo "$METADATA" | grep "width" | cut -d= -f2)
HEIGHT=$(echo "$METADATA" | grep "height" | cut -d= -f2)

# Extract audio codec
AUDIO_CODEC=$(ffprobe -v error -select_streams a:0 -show_entries stream=codec_name -of default=noprint_wrappers=1:nokey=1 "$VIDEO_FILE" | head -n 1)

# Print current state
echo "======================================"
echo "   VIDEO METADATA REPORT"
echo "======================================"
echo "File:        $VIDEO_FILE"
echo "Video Codec: $CODEC"
echo "Profile:     $PROFILE"
echo "Pixel Format:$PIX_FMT"
echo "Resolution:  ${WIDTH}x${HEIGHT}"
echo "Audio Codec: $AUDIO_CODEC"
echo "--------------------------------------"

# Compatibility Logic
FRIENDLY=true
REASON=""

# 1. Check Codec and 10-bit H.264 trap
if [ "$CODEC" = "h264" ]; then
    if [[ "$PIX_FMT" == *"10"* ]] || [[ "$PROFILE" == *"10"* ]]; then
        FRIENDLY=false
        REASON="DeoVR/Quest mobile hardware crashes (grey screen) on 10-bit H.264 video. Must convert to 8-bit H.264 or 10-bit H.265."
    fi
elif [ "$CODEC" != "hevc" ] && [ "$CODEC" != "av1" ] && [ "$CODEC" != "vp9" ]; then
    FRIENDLY=false
    REASON="Unknown or unsupported video codec ($CODEC)."
fi

# 2. Check Resolution limits (Quest 2/3 hardware ceiling)
if [ -n "$WIDTH" ] && [ "$WIDTH" -gt 8192 ]; then
    FRIENDLY=false
    REASON="Resolution width ($WIDTH) exceeds the hardware decoding ceiling (8K max)."
fi

# 3. Check Audio
if [ "$AUDIO_CODEC" != "aac" ] && [ "$AUDIO_CODEC" != "mp3" ] && [ -n "$AUDIO_CODEC" ]; then
    FRIENDLY=false
    REASON="Audio codec ($AUDIO_CODEC) might freeze DeoVR. Standardize to aac."
fi

# Output Results
if [ "$FRIENDLY" = true ]; then
    echo " RESULT:  DeoVR Friendly!"
    echo "Action:     No full re-encode needed. Just use 'ffmpeg -i input -c copy' if changing containers."
else
    echo " RESULT: ❌ NOT DeoVR Friendly!"
    echo "Issue:      $REASON"
    echo "--------------------------------------"
    echo "Suggested FFmpeg Fix Command:"
    
    # Generate the optimal hardware-accelerated fix command for Mac (VideoToolbox)
    if [ "$CODEC" = "h264" ] && [[ "$PIX_FMT" == *"10"* ]]; then
        echo "ffmpeg -i \"$VIDEO_FILE\" -c:v h264_videotoolbox -pix_fmt yuv420p -c:a aac \"fixed_${VIDEO_FILE%.*}.mp4\""
    else
        echo "ffmpeg -i \"$VIDEO_FILE\" -c:v hevc_videotoolbox -pix_fmt yuv420p10le -c:a aac \"fixed_${VIDEO_FILE%.*}.mp4\""
    fi
fi
echo "======================================"


