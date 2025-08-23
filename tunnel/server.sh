#!/bin/bash
while true; do
  	echo -e "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nHello" | nc -l 127.0.0.1 9000
done
