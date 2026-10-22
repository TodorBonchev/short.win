import hashlib
import json
from pathlib import Path
import struct

root = Path('/home/todor_dev/Projects/short.win-release-devnet')
fresh = root / 'target/program-readiness-2026-10-22/deploy/shortwin.so'
canonical = Path('/home/todor_dev/Projects/short.win/target/deploy/shortwin.so')
mock = root / 'target/program-readiness-2026-10-22/deploy/mock_cpamm.so'

def sha(blob):
    return hashlib.sha256(blob).hexdigest()

def sections(blob):
    offset = struct.unpack_from('<Q', blob, 40)[0]
    size, count, str_index = struct.unpack_from('<HHH', blob, 58)
    rows = [struct.unpack_from('<IIQQQQIIQQ', blob, offset + i * size) for i in range(count)]
    strings = blob[rows[str_index][4]:rows[str_index][4] + rows[str_index][5]]
    result = {}
    for row in rows[1:]:
        name = strings[row[0]:].split(b'\0', 1)[0].decode()
        data = blob[row[4]:row[4] + row[5]]
        result[name] = dict(address=row[3], offset=row[4], size=row[5], sha256=sha(data))
    return result

a, b = canonical.read_bytes(), fresh.read_bytes()
assert sha(a) == 'd6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc'
assert len(a) == len(b)
diffs = [dict(offset=i, old=x, new=y) for i, (x, y) in enumerate(zip(a, b)) if x != y]
flags = struct.unpack_from('<I', b, 48)[0]
assert flags == 3
assert b'shortwin:test-venue:mock-cpamm' not in b
assert b.count(b'=======BEGIN SECURITY.TXT V1=======') == 1
result = dict(
    source_commit='b31c02a091008da18c39d7836c8e0e9d6d018973',
    canonical=dict(path=str(canonical), bytes=len(a), sha256=sha(a),
                   trimmed_sha256=sha(a.rstrip(b'\0')), sections=sections(a)),
    fresh=dict(path=str(fresh), bytes=len(b), sha256=sha(b),
               trimmed_sha256=sha(b.rstrip(b'\0')), e_flags=flags, sections=sections(b),
               test_venue_marker_absent=True, security_txt_marker_count=1),
    mock=dict(path=str(mock), bytes=mock.stat().st_size, sha256=sha(mock.read_bytes())),
    byte_differences=diffs,
    attribution='Single changed immediate in .text: Anchor ZeroAmount error source line moves 72→73 at add_margin require!, matching comment-only source line shift. See retained disassembly and macro source. This is not canonical byte identity or a waiver of final-byte gates.',
)
assert diffs == [dict(offset=330428, old=72, new=73)]
print(json.dumps(result, indent=2, ensure_ascii=False))
