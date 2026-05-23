import struct
import zlib
import os

def create_png(width, height, color=(59, 130, 246, 255)):
    def chunk(chunk_type, data):
        c = chunk_type + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    
    header = b'\x89PNG\r\n\x1a\n'
    ihdr = chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
    
    raw = b''
    for y in range(height):
        raw += b'\x00'
        for x in range(width):
            raw += bytes(color)
    
    idat = chunk(b'IDAT', zlib.compress(raw))
    iend = chunk(b'IEND', b'')
    
    return header + ihdr + idat + iend

os.makedirs('src-tauri/icons', exist_ok=True)

sizes = {'32x32': 32, '128x128': 128, '128x128@2x': 256}
for name, size in sizes.items():
    with open(f'src-tauri/icons/{name}.png', 'wb') as f:
        f.write(create_png(size, size))
    print(f'Created {name}.png ({size}x{size})')

print('Done')
