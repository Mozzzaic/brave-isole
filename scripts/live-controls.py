#!/usr/bin/env python3
"""Temporary synthetic loopback witnesses, never part of the shipped Rust bridge."""
import json,os,selectors,socket,sys,time
from pathlib import Path
seed=Path(sys.argv[1]);destination=Path(sys.argv[2]);request=json.loads(seed.read_text());selector=selectors.DefaultSelector();sockets=[]
try:
 for family,address,key in [(socket.AF_INET,'127.0.0.1','tcp4'),(socket.AF_INET6,'::1','tcp6')]:
  # TCP and UDP may use the same numeric port; neither accepts commands or reads files.
  tcp=socket.socket(family,socket.SOCK_STREAM);tcp.bind((address,0));tcp.listen(8);tcp.setblocking(False);port=tcp.getsockname()[1];sockets.append(tcp);selector.register(tcp,selectors.EVENT_READ,'tcp')
  udp=socket.socket(family,socket.SOCK_DGRAM);udp.bind((address,port));udp.setblocking(False);sockets.append(udp);selector.register(udp,selectors.EVENT_READ,'udp')
  request[key]=f'{address}:{port}' if family==socket.AF_INET else f'[{address}]:{port}'
 temporary=destination.with_suffix('.tmp');temporary.write_text(json.dumps(request));temporary.chmod(0o600);os.replace(temporary,destination)
 deadline=time.monotonic()+1800
 while destination.exists() and time.monotonic()<deadline:
  if json.loads(destination.read_text()).get('nonce')!=request['nonce']:break
  for key,_ in selector.select(timeout=0.5):
   if key.data=='tcp':connection,_=key.fileobj.accept();connection.close()
   else:key.fileobj.recvfrom(1024)
finally:
 selector.close()
 for item in sockets:item.close()
