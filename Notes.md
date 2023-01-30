# Build gtk in windows

https://github.com/wingtk/gvsbuild
gvsbuild build gtk3 glib-networking gstreamer gst-plugins-\*
install pkg-config-lite
SET PKG_CONFIG_PATH=C:\gtk-build\gtk\x64\release\lib\pkgconfig

# Gtk4 rust book

https://gtk-rs.org/gtk4-rs/stable/latest/book/installation_windows.html

<!-- messon certificate error: install the intermediate R3 certificate from let's encrypt -->
<!-- set PKG_CONFIG_PATH=C:\gnome\lib\pkgconfig -->
<!-- pip install meson ninja -->

<!-- git clone https://gitlab.gnome.org/GNOME/gtk.git --depth 1
git clone https://gitlab.gnome.org/GNOME/libxml2.git --depth 1
git clone https://gitlab.gnome.org/GNOME/librsvg.git --depth 1
git clone https://gitlab.freedesktop.org/gstreamer/gstreamer.git --depth 1

cd gtk
meson setup builddir --prefix=C:/gnome -Dbuild-tests=false -Dmedia-gstreamer=disabled
meson install -C builddir
cd ..

cd libxml2
cmake -S . -B build -D CMAKE_BUILD_TYPE=Release -D CMAKE_INSTALL_PREFIX=C:\gnome -D LIBXML2_WITH_ICONV=OFF -D LIBXML2_WITH_LZMA=OFF -D LIBXML2_WITH_PYTHON=OFF -D LIBXML2_WITH_ZLIB=OFF
cmake --build build --config Release
cmake --install build
cd ..

cd librsvg/win32
where python
nmake /f generate-msvc.mak generate-nmake-files PYTHON=<output from last command>
xcopy /s C:\gnome\include\cairo C:\gnome\include
nmake /f Makefile.vc CFG=release install PREFIX=C:\gnome
cd ..

cd gstreamer
meson setup builddir --prefix=C:/gnome --reconfigure
meson install -C builddir
meson test -C build
cd ..


 -->

# Some command line tests with gstreamer

gst-launch-1.0 ximagesrc xid=0x60000f ! video/x-raw,framerate=5/1 ! videoconvert ! theoraenc ! oggmux ! filesink location=desktop.ogg

gst-launch-1.0 ximagesrc xid=0x60000f ! video/x-raw,framerate=5/1 ! videoconvert ! queue ! x264enc pass=5 quantizer=26 speed-preset=6 ! mp4mux fragment-duration=500 ! filesink location="capture.mp4"

gst-launch-1.0 ximagesrc xid=0x2c00007 ! video/x-raw,framerate=30/1 ! videoscale method=0 ! videoconvert ! x264enc ! mp4mux ! filesink location=output2.mp4

## TCP connection

# linux server

gst-launch-1.0 ximagesrc xid=0x2c00007 ! queue ! videoconvert ! video/x-raw,framerate=30/1 ! jpegenc ! multipartmux ! tcpserversink host=0.0.0.0 port=7001

# windows client

gst-launch-1.0 tcpclientsrc host=192.168.100.202 port=7001 ! multipartdemux ! jpegdec ! glimagesink

## SSH tunnel

# server:

# run app in hidden display

Xvfb :101 &

# Xvfb +extension GLX +extension Composite -screen 0 8192x4096x24+32 -nolisten tcp -noreset -auth /run/user/1000/gdm/Xauthority -dpi 96 :101 &

export DISPLAY=:101
xterm &

# find window id

xwininfo -root -tree

gst-launch-1.0 ximagesrc xid=0x40000d ! queue ! videoconvert ! video/x-raw,framerate=30/1 ! jpegenc ! multipartmux ! tcpserversink host=127.0.0.1 port=7001

# client, terminal 1

ssh -L 7001:localhost:7001 san@192.168.100.202

# client, terminal 2

gst-launch-1.0 tcpclientsrc host=127.0.0.1 port=7001 ! multipartdemux ! jpegdec ! glimagesink

# server, send keyborad commands

xdotool search -class xterm windowraise && xdotool type "ls -l" && xdotool key Return

# Optimization

server:
gst-launch-1.0 ximagesrc use-damage=0 \

>          ! video/x-raw,framerate=30/1 \
>          ! videoscale method=0 \
>          ! video/x-raw \
>          ! videoconvert \
>          ! x264enc tune=zerolatency\
>          ! rtph264pay \
>          ! udpsink host=192.168.1.5 port=5601
>
> - receiver
>   gst-launch-1.0 -e udpsrc port=5601 caps="application/x-rtp,
>   media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264,
>   payload=96" \
>    ! rtpjitterbuffer \
>    ! rtph264depay \
>    ! avdec_h264 \
>    ! videoconvert \
>    ! videoscale \
>    ! ximagesink sync=false

gst-launch-1.0 v4l2src ! \
 video/x-raw,width=640,height=480 ! \
 x264enc tune=zerolatency byte-stream=true \
 bitrate=3000 threads=2 ! \
 h264parse config-interval=1 ! \
 rtph264pay ! udpsink host=127.0.0.1 port=5000
