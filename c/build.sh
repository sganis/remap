
#!/bin/sh
gcc client-gui.c -o client-gui \
    `pkg-config --cflags --libs gstreamer-video-1.0 gtk+-3.0 gstreamer-1.0`