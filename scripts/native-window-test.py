"""Linux-only, isolated native WebKit/IPC/close smoke test. Requires Xvfb and xdotool.
Optional screenshot is generated from XWD using only Python's standard library.
"""
import ctypes as c
import json
import os
import pathlib
import shutil
import socket
import struct
import subprocess as sp
import tempfile
import time
import urllib.request
import zlib

root = pathlib.Path(__file__).resolve().parent.parent
cli = pathlib.Path(os.environ.get('MAGPIE_TEST_BINARY', root / 'target/debug/magpie')).resolve()
desktop = pathlib.Path(os.environ.get('MAGPIE_TEST_DESKTOP', root / 'target/release/magpie-desktop')).resolve()
xvfb = os.environ.get('MAGPIE_XVFB', 'Xvfb')
xdotool = os.environ.get('MAGPIE_XDOTOOL', 'xdotool')
display = os.environ.get('MAGPIE_TEST_DISPLAY', ':198')
processes = []

def until(fn, seconds=45):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            result = fn()
            if result: return result
        except (OSError, ValueError, sp.CalledProcessError): pass
        time.sleep(.2)
    raise AssertionError('Timed out waiting for native app')

def close_window(window):
    class Data(c.Union): _fields_ = [('l', c.c_long * 5)]
    class Message(c.Structure):
        _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int),
                    ('display', c.c_void_p), ('window', c.c_ulong), ('message_type', c.c_ulong),
                    ('format', c.c_int), ('data', Data)]
    class Event(c.Union): _fields_ = [('msg', Message), ('pad', c.c_long * 24)]
    x = c.CDLL('libX11.so.6')
    x.XOpenDisplay.argtypes = [c.c_char_p]; x.XOpenDisplay.restype = c.c_void_p
    x.XInternAtom.argtypes = [c.c_void_p, c.c_char_p, c.c_int]; x.XInternAtom.restype = c.c_ulong
    x.XSendEvent.argtypes = [c.c_void_p, c.c_ulong, c.c_int, c.c_long, c.POINTER(Event)]
    x.XFlush.argtypes = [c.c_void_p]; x.XCloseDisplay.argtypes = [c.c_void_p]
    d = x.XOpenDisplay(display.encode()); assert d
    e = Event(); e.msg.type = 33; e.msg.send_event = 1; e.msg.display = d
    e.msg.window = window; e.msg.message_type = x.XInternAtom(d, b'WM_PROTOCOLS', 0)
    e.msg.format = 32; e.msg.data.l[0] = x.XInternAtom(d, b'WM_DELETE_WINDOW', 0)
    x.XSendEvent(d, window, 0, 0, c.byref(e)); x.XFlush(d); x.XCloseDisplay(d)

def screenshot(env):
    destination = os.environ.get('MAGPIE_TEST_SCREENSHOT')
    if not destination: return
    data = sp.check_output(['xwd', '-root', '-silent'], env=env)
    h = struct.unpack('>25I', data[:100]); width, height, bpp, stride = h[4], h[5], h[11], h[12]
    assert bpp in (24, 32), f'Unexpected XWD header: {h}'
    pixels = data[h[0] + h[19] * 12:]
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        row = pixels[y * stride:y * stride + width * (bpp // 8)]
        for i in range(0, len(row), bpp // 8):
            pixel = int.from_bytes(row[i:i+bpp//8], 'little' if h[7] == 0 else 'big')
            raw.extend(((pixel >> 16) & 255, (pixel >> 8) & 255, pixel & 255))
    def chunk(tag, body):
        return struct.pack('>I', len(body)) + tag + body + struct.pack('>I', zlib.crc32(tag + body))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>2I5B', width, height, 8, 2, 0, 0, 0))
    pathlib.Path(destination).parent.mkdir(parents=True, exist_ok=True)
    pathlib.Path(destination).write_bytes(png + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))

with tempfile.TemporaryDirectory(prefix='magpie-native-') as data:
    env = {**os.environ, 'MAGPIE_HOME': data, 'DISPLAY': display, 'GDK_BACKEND': 'x11',
           'WEBKIT_DISABLE_DMABUF_RENDERER': '1'}
    env.pop('WAYLAND_DISPLAY', None); env.pop('MAGPIE_API_KEY', None)
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0)); port = s.getsockname()[1]
    url = f'http://127.0.0.1:{port}'
    log = open(pathlib.Path(data) / 'test.log', 'w')
    def launch(args):
        p = sp.Popen([str(a) for a in args], env=env, stdout=log, stderr=log)
        processes.append(p); return p
    def api(path, body=None):
        token = (pathlib.Path(data) / 'admin.token').read_text().strip()
        request = urllib.request.Request(url + path, data=None if body is None else json.dumps(body).encode(),
            headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'},
            method='PUT' if path == '/v1/settings' and body else 'POST' if body is not None else 'GET')
        with urllib.request.urlopen(request, timeout=2) as response:
            text = response.read(); return json.loads(text) if text else True
    try:
        launch([xvfb, display, '-screen', '0', '1280x900x24', '-ac'])
        seed = launch([cli, 'serve', '--port', port]); until(lambda: api('/v1/status'))
        api('/v1/admin/shutdown', {}); assert seed.wait(15) == 0
        for keep in (False, True):
            app = launch([desktop]); until(lambda: api('/v1/status'))
            window = until(lambda: sp.check_output([xdotool, 'search', '--onlyvisible', '--name', '^Magpie$'], env=env, stderr=sp.DEVNULL).splitlines())
            assert app.poll() is None
            if keep:
                settings = api('/v1/settings')['settings']; settings['general']['keep_harness_running'] = True
                api('/v1/settings', settings)
            time.sleep(3)
            if not keep: screenshot(env)
            close_window(int(window[-1])); assert app.wait(20) == 0
            if keep:
                assert api('/v1/status'), 'Opted-in background service must survive window exit'
                api('/v1/admin/shutdown', {})
            else:
                try: api('/v1/status')
                except OSError: pass
                else: raise AssertionError('Default close must stop harness')
        print('PASS: native WebKit startup, authenticated IPC, default close stops service, opted-in background survives UI exit')
    except Exception:
        log.flush(); print((pathlib.Path(data) / 'test.log').read_text())
        print('Process exits:', [p.poll() for p in processes])
        try: screenshot(env)
        except Exception as error: print('Screenshot failed:', error)
        raise
    finally:
        try: api('/v1/admin/shutdown', {})
        except OSError: pass
        for p in reversed(processes):
            if p.poll() is None: p.terminate()
        for p in processes:
            try: p.wait(10)
            except sp.TimeoutExpired: p.kill(); p.wait()
        log.close()
