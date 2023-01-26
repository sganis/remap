#!/bin/sh

DISPLAY=:101
APP=gnome-terminal
export DISPLAY=$DISPLAY

Xvfb +extension GLX +extension Composite \
    -screen 0 8192x4096x24+32 \
    -nolisten tcp -noreset \
    -auth /run/user/1000/gdm/Xauthority \
    -dpi 96 $DISPLAY &

while ! $(ps aux |grep Xvfb |grep $DISPLAY >/dev/null); do
    echo waiting for X
    sleep 1
done

$APP &
PID=$(echo $!)
echo "PID=$PID"

# ID=$(xwininfo -root -tree |grep $APP |head -1 |awk '{print $1}')
ID=$(xdotool search --pid $PID |head -1)
while [ -z "$ID" ]; do
    echo waiting for app
    sleep 1
    # ID=$(xwininfo -root -tree |grep $APP |head -1 |awk '{print $1}')
    ID=$(xdotool search --pid $PID |head -1)
done
echo $ID

# full desktop
# ID=
ID="xid=$ID"

gst-launch-1.0 ximagesrc $ID use-damage=0 ! queue ! videoconvert \
    ! video/x-raw,framerate=24/1 ! jpegenc ! multipartmux \
    ! tcpserversink host=127.0.0.1 port=7001

# gst-launch-1.0 ximagesrc $ID use-damage=0 ! queue ! videoconvert \
#     ! video/x-raw,framerate=30/1 ! jpegenc ! multipartmux \
#     ! udpsink host=127.0.0.1 port=7001


