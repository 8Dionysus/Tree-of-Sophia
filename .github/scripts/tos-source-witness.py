"""INERT proposed metadata witness. Only an externally selected immutable driver may run.

No imports perform host IO. No compiler, subprocess, policy write, namespace or ticket.
Source bodies are hashed only. Raw policy endpoints are NEVER opened.
"""
import ctypes
import errno
import hashlib
import json
import os
import re
import signal
import stat
import struct
import sys
import time

INPUT_CAP = 8388608
SECONDS = 5
OUTPUT_CAP = 524288
OP_CAP = 30000
STAT_DEBIT = 256  # Conservative fixed-size metadata debit for selected LinuxX64 ABI.
POLICY = '/sys/kernel/security/apparmor/policy'
RAW_CONTAINER = POLICY + '/raw_data'
PROFILE_LINKS = ('raw_data', 'raw_sha256', 'raw_abi')
HEX64 = re.compile(r'[0-9a-f]{64}\Z')


# BEGIN GENERATED FIXED PROTOCOL
"""Pure fixed-status codec. No filesystem, clocks, exception text or subprocess."""
STATUS_RESERVE = 512
STATUS_LINE_CAP = 192
REFUSAL_CODES = {
    "bad-field-cap": 20,
    "bad-limit": 21,
    "conditional-drift": 22,
    "config-drift": 23,
    "custody": 24,
    "deadline": 25,
    "declared-size": 26,
    "descriptor-cleanup": 27,
    "directory-cache-binding": 28,
    "directory-cache-drift": 29,
    "directory-child-unreviewed": 30,
    "directory-count": 31,
    "directory-descriptor-cap": 32,
    "directory-drift": 33,
    "directory-duplicate": 34,
    "directory-header": 35,
    "directory-membership": 36,
    "directory-record": 37,
    "empty-local-drift": 38,
    "encoding": 39,
    "entry-name": 40,
    "executable-drift": 41,
    "executable-hash": 42,
    "existing-timer": 43,
    "feature-path": 44,
    "feature-total": 45,
    "feature-value": 46,
    "features-drift": 47,
    "features-empty": 48,
    "field-budget": 49,
    "held-executable-cap": 50,
    "held-file-drift": 51,
    "input-budget": 52,
    "link-encoding": 53,
    "link-size": 54,
    "mandatory-absent": 55,
    "metadata-type": 56,
    "named-directory-drift": 57,
    "named-file-drift": 58,
    "native-abi": 59,
    "native-buffer": 60,
    "native-return": 61,
    "operation-budget": 62,
    "output-budget": 63,
    "path-shape": 64,
    "policy-inventory-drift": 65,
    "policy-revision-drift": 66,
    "private-local-nonempty": 67,
    "profile-count": 68,
    "profile-link-directory-forbidden": 69,
    "profile-link-type": 70,
    "profile-sha": 71,
    "profile-sha-missing": 72,
    "raw-abi": 73,
    "raw-alias-drift": 74,
    "raw-directory-forbidden": 75,
    "raw-endpoint-drift": 76,
    "raw-link": 77,
    "raw-link-escape": 78,
    "raw-link-layout": 80,
    "raw-metadata-drift": 81,
    "raw-open-forbidden": 82,
    "raw-selector": 83,
    "raw-selector-set": 84,
    "raw-size-unavailable": 85,
    "reader-contract": 86,
    "revision": 87,
    "selected-context": 88,
    "source-drift": 89,
    "source-hash": 90,
    "total-output-budget": 91,
    "tree-depth": 92,
    "tree-directories": 93,
    "tree-files": 94,
    "tree-link": 95,
    "type": 96,
    "unreviewed-directory": 97,
    "writer-contract": 98
}
EARLY = {
    3: ('entry','selected-context','not-entered'),
    4: ('payload','packed-bound','not-entered'),
    5: ('payload','payload-bound','not-entered'),
    6: ('payload','payload-hash','not-entered'),
    7: ('payload','payload-shape','not-entered'),
    8: ('payload','payload-type','not-entered'),
    9: ('payload','untyped-failure','not-entered'),
    10: ('binding','source-identity','not-entered'),
    11: ('binding','platform-binding','not-entered'),
    12: ('binding','image-version-metadata','not-entered'),
    13: ('binding','sudo-environment-drift','not-entered'),
    14: ('binding','untyped-failure','not-entered'),
    15: ('collector-load','untyped-failure','not-entered'),
    16: ('collector-call','untyped-failure','unknown'),
    17: ('driver','untyped-failure','unknown'),
}
REFUSAL_CODES.update({'policy-entry-not-directory': 101, 'policy-entry-link-loop': 102, 'policy-entry-access-denied': 103, 'policy-entry-not-permitted': 104, 'policy-entry-not-found': 105, 'policy-entry-other-io': 106, 'revision-read-not-ready': 107, 'revision-read-access-denied': 108, 'revision-read-other-io': 109})
EARLY.update({101: ('policy-entry', 'not-directory', 'outer-ok'), 102: ('policy-entry', 'link-loop', 'outer-ok'), 103: ('policy-entry', 'access-denied', 'outer-ok'), 104: ('policy-entry', 'not-permitted', 'outer-ok'), 105: ('policy-entry', 'not-found', 'outer-ok'), 106: ('policy-entry', 'other-io', 'outer-ok'), 107: ('revision-read', 'not-ready', 'outer-ok'), 108: ('revision-read', 'access-denied', 'outer-ok'), 109: ('revision-read', 'other-io', 'outer-ok')})
REFUSAL_CODES.update({'kernel-policy-parent-fs': 18, 'kernel-policy-link-kind': 19, 'kernel-policy-link-custody': 117, 'kernel-policy-link-drift': 118, 'kernel-policy-target-fs': 119, 'kernel-policy-root-binding': 120, 'kernel-policy-root-state': 121, 'kernel-statfs-abi': 122, 'kernel-statfs-io': 123, 'kernel-policy-parent-drift': 124})
EARLY.update({18: ('kernel-entry', 'parent-filesystem', 'outer-ok'), 19: ('kernel-entry', 'link-kind', 'outer-ok'), 117: ('kernel-entry', 'link-custody', 'outer-ok'), 118: ('kernel-entry', 'link-drift', 'outer-ok'), 119: ('kernel-entry', 'target-filesystem', 'outer-ok'), 120: ('kernel-entry', 'root-binding', 'outer-ok'), 121: ('kernel-entry', 'root-state', 'outer-ok'), 122: ('kernel-entry', 'statfs-abi', 'outer-ok'), 123: ('kernel-entry', 'statfs-io', 'outer-ok'), 124: ('kernel-entry', 'parent-drift', 'outer-ok')})
CUSTODY_STATUS = {200: ('collector', 'custody-c01-uid', 'outer-ok'), 201: ('collector', 'custody-c01-write', 'outer-ok'), 202: ('collector', 'custody-c02-uid', 'outer-ok'), 203: ('collector', 'custody-c02-write', 'outer-ok'), 204: ('collector', 'custody-c03-uid', 'outer-ok'), 205: ('collector', 'custody-c03-write', 'outer-ok'), 206: ('collector', 'custody-c04-uid', 'outer-ok'), 207: ('collector', 'custody-c04-write', 'outer-ok'), 208: ('collector', 'custody-c05-uid', 'outer-ok'), 209: ('collector', 'custody-c05-write', 'outer-ok'), 210: ('collector', 'custody-c06-uid', 'outer-ok'), 211: ('collector', 'custody-c06-write', 'outer-ok'), 212: ('collector', 'custody-c07-uid', 'outer-ok'), 213: ('collector', 'custody-c07-write', 'outer-ok'), 214: ('collector', 'custody-c08-uid', 'outer-ok'), 215: ('collector', 'custody-c08-write', 'outer-ok'), 216: ('collector', 'custody-c09-uid', 'outer-ok'), 217: ('collector', 'custody-c09-write', 'outer-ok'), 218: ('collector', 'custody-c10-uid', 'outer-ok'), 219: ('collector', 'custody-c10-write', 'outer-ok'), 220: ('collector', 'custody-c11-uid', 'outer-ok'), 221: ('collector', 'custody-c11-write', 'outer-ok'), 222: ('collector', 'custody-c12-uid', 'outer-ok'), 223: ('collector', 'custody-c12-write', 'outer-ok'), 224: ('collector', 'custody-c13-uid', 'outer-ok'), 225: ('collector', 'custody-c13-write', 'outer-ok'), 226: ('collector', 'custody-c14-uid', 'outer-ok'), 227: ('collector', 'custody-c14-write', 'outer-ok'), 228: ('collector', 'custody-c15-uid', 'outer-ok'), 229: ('collector', 'custody-c15-write', 'outer-ok')}
EARLY.update(CUSTODY_STATUS)
PAYLOAD_CODES = {'packed-bound':4,'payload-bound':5,'payload-hash':6,'payload-shape':7,'payload-type':8}
BINDING_CODES = {'source-identity':10,'platform-binding':11,'image-version-metadata':12,'sudo-environment-drift':13}


def literal_value_error(exc, table, fallback):
    if type(exc) is ValueError and len(exc.args)==1 and type(exc.args[0]) is str:
        return table.get(exc.args[0],fallback)
    return fallback


def refusal_exit(exc, refusal_type):
    if type(exc) is refusal_type and len(exc.args)==1 and type(exc.args[0]) is str:
        if exc.args[0]=='custody':
            #Only our fixed integer discriminator is decoded; original args stay.
            try:
                marker=exc.__dict__.get('_custody_discriminator')
            except MemoryError:
                marker=None  #Diagnostic allocation cannot replace original24.
            if type(marker) is int and 0<=marker<30:
                return 200+marker
        return REFUSAL_CODES.get(exc.args[0],100)
    if isinstance(exc,OSError):
        return 99
    return 100


def cleanup_exit(primary, mask):
    #mask bits1/2/4 mean descriptor-close/disarm/handler-restore failure.
    #A cleanup failure deliberately overrides the primary reason; do not infer it.
    if type(mask) is not int or not 0<=mask<=7:
        return 100
    return 109+mask if mask else primary


def status_record(raw):
    code = None
    if type(raw) is str and 1<=len(raw)<=3 and raw.isascii() and raw.isdecimal():
        value = int(raw)
        if 0<=value<=255 and str(value)==raw:
            code = value
    shown = str(code) if code is not None else 'unknown'
    if code==79:
        phase,reason,cleanup = 'complete','none','outer-ok'
    elif code in EARLY:
        phase,reason,cleanup = EARLY[code]
    elif code in REFUSAL_CODES.values():
        phase,reason,cleanup = 'collector',next(k for k,v in REFUSAL_CODES.items() if v==code),'outer-ok'
    elif code in (99,100):
        phase,reason,cleanup = 'collector',('os-error' if code==99 else 'untyped-failure'),'outer-ok'
    elif code is not None and 110<=code<=116:
        phase,cleanup = 'cleanup','outer-failed'
        reason = ('fs' if (code-109)&1 else '') + ('-timer' if (code-109)&2 else '') + ('-handler' if (code-109)&4 else '')
        reason = reason.lstrip('-')
    else:
        phase,reason,cleanup = 'transport','untyped-or-abrupt','unknown'
    line = ('TOS_WITNESS_STATUS_V1 child='+shown+' phase='+phase+' reason='+reason+' cleanup='+cleanup+'\n').encode('ascii')
    if len(line)>STATUS_LINE_CAP:
        raise ValueError('fixed-status-bound')
    return line


def wrapper_unverified_record():
    return b'TOS_WITNESS_STATUS_V1 child=79 phase=wrapper reason=output-unverified cleanup=outer-ok\n'
# END GENERATED FIXED PROTOCOL


class Refusal(Exception):
    """Codes are fixed nonsecret literals; never include observed content/path names."""


def require(value, code):
    if not value:
        raise Refusal(code)


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Ledger:
    def __init__(self, clock=time.monotonic, cap=INPUT_CAP, seconds=SECONDS):
        require(0 < cap <= INPUT_CAP and 0 < seconds <= SECONDS, 'bad-limit')
        self.clock = clock
        self.deadline = clock() + seconds
        self.cap = cap
        self.used = 0
        self.ops = 0

    def check(self):
        require(self.clock() < self.deadline, 'deadline')

    def reserve(self, count):
        self.check()
        require(type(count) is int and count >= 0 and count <= self.cap - self.used,
                'input-budget')

    def charge(self, count):
        self.reserve(count)
        self.used += count

    def call(self, fn, *args, **kwargs):
        self.check()
        require(self.ops < OP_CAP, 'operation-budget')
        self.ops += 1
        try:
            value = fn(*args, **kwargs)
        except BaseException:
            self.check()
            raise
        self.check()
        return value

    def read(self, reader, field_cap):
        require(type(field_cap) is int and 0 <= field_cap <= INPUT_CAP, 'bad-field-cap')
        result = bytearray()
        while True:
            self.check()
            remaining = self.cap - self.used
            require(remaining > 0, 'input-budget')
            count = min(65536, field_cap - len(result) + 1, remaining)
            self.reserve(count)  # BEFORE actual read, including the one-byte EOF probe.
            require(self.ops < OP_CAP, 'operation-budget')
            self.ops += 1
            try:
                data = reader(count)
            except BaseException:
                self.check()
                raise
            require(type(data) is bytes and len(data) <= count, 'reader-contract')
            # Charge successful bytes even when deadline/field checks then refuse.
            self.used += len(data)
            self.check()
            require(len(result) + len(data) <= field_cap, 'field-budget')
            result.extend(data)
            if not data:
                return bytes(result)

    def acquire(self, operation, *args, **kwargs):
        """Close a newly returned FD even if the post-operation deadline expires."""
        self.check()
        require(self.ops < OP_CAP, 'operation-budget')
        self.ops += 1
        try:
            fd = operation(*args, **kwargs)
        except BaseException:
            self.check()
            raise
        try:
            self.check()
            return fd
        except BaseException:
            os.close(fd)
            raise

    def metadata(self, operation, cap, code):
        """Prospectively reserve the kernel maximum; debit actual returned name/link bytes.

        On an error without a returned value, debit the entire reservation and abort.
        EOF of a directory is the only zero-byte successful terminal case.
        """
        self.reserve(cap)
        require(self.ops < OP_CAP, 'operation-budget')
        self.ops += 1
        try:
            value = operation()
        except StopIteration:
            self.check()
            raise
        except BaseException:
            self.used += cap
            self.check()
            raise
        require(type(value) is str, 'metadata-type')
        size = len(os.fsencode(value))
        require(size <= cap, code)
        self.used += size
        self.check()
        return value


def revision_frame(budget, reader):
    """One complete bounded numeric kernel record; never read-until-EOF or retry."""
    budget.reserve(65)  #Original ledger, prospective BEFORE the single read.
    require(budget.ops < OP_CAP, 'operation-budget')
    budget.ops += 1
    try:
        data=reader(65)
    except OSError as exc:
        budget.check()
        code=exc.errno if type(exc.errno) is int else None
        reason=('revision-read-not-ready' if code in (errno.EAGAIN,errno.EWOULDBLOCK)
                else 'revision-read-access-denied' if code in (errno.EACCES,errno.EPERM)
                else 'revision-read-other-io')
        raise Refusal(reason) from None
    except BaseException:
        budget.check()
        raise
    require(type(data) is bytes and len(data)<=65, 'reader-contract')
    budget.used += len(data)  #Debit even if a later field/deadline check refuses.
    budget.check()
    require(len(data)<=64, 'field-budget')
    return data


def components(path):
    require(type(path) is str and path.startswith('/') and len(path) <= 4096,
            'path-shape')
    parts = path[1:].split('/')
    require(parts and all(p and p not in ('.', '..') and '\x00' not in p
                          and len(p.encode('utf-8')) <= 255 for p in parts), 'path-shape')
    return parts


def directory_allowed(path):
    """Only the raw-data CONTAINER may be opened as a raw_data directory.

    O_DIRECTORY is still an open: opaque endpoints and profile-link leaves may
    never be opened/traversed, even if a wrong-type node appears root-owned.
    Metadata-only info on the final leaf remains possible through its parent.
    """
    prefix = ''
    for part in components(path):
        prefix += '/' + part
        require(part != 'raw_data' or prefix == RAW_CONTAINER, 'raw-directory-forbidden')
        require(not (prefix.startswith(POLICY+'/profiles/') and part in PROFILE_LINKS),
                'profile-link-directory-forbidden')


def custody_refusal(site, predicate):
    #Preserve original exception class/args. No path/UID/mode/text is retained.
    exc=Refusal('custody')
    if type(site) is int and 1<=site<=15 and type(predicate) is int and 0<=predicate<2:
        try:
            exc._custody_discriminator=2*(site-1)+predicate
        except MemoryError:
            pass  #Keep the original refusal, fall back to legacy24.
    raise exc


def custody(s, directory=False, *, site=None):
    #Same short-circuit predicate order; no mode observation after failed UID.
    if not s.st_uid == 0:
        custody_refusal(site,0)
    if s.st_mode & 0o022:
        custody_refusal(site,1)
    require(stat.S_ISDIR(s.st_mode) if directory else stat.S_ISREG(s.st_mode), 'type')


def identity(s):
    return tuple(getattr(s, key) for key in
                 ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid', 'st_size',
                  'st_mtime_ns', 'st_ctime_ns'))




class KernelStatFS(ctypes.Structure):
    _fields_=[('kind',ctypes.c_long),('block_size',ctypes.c_long),
              ('blocks',ctypes.c_ulong),('free_blocks',ctypes.c_ulong),
              ('available_blocks',ctypes.c_ulong),('files',ctypes.c_ulong),
              ('free_files',ctypes.c_ulong),('fsid',ctypes.c_int*2),
              ('name_length',ctypes.c_long),('fragment_size',ctypes.c_long),
              ('flags',ctypes.c_long),('spare',ctypes.c_long*4)]


def kernel_fs_kind(native,fd):
    #Fixed LinuxX64/glibc statfs ABI. No raw syscall number or path selector.
    require(ctypes.sizeof(KernelStatFS)==120 and ctypes.alignment(KernelStatFS)==8,
            'kernel-statfs-abi')
    result=KernelStatFS()
    call=native.lib.fstatfs
    call.argtypes=[ctypes.c_int,ctypes.POINTER(KernelStatFS)]
    call.restype=ctypes.c_int
    if call(fd,ctypes.byref(result))!=0:
        raise Refusal('kernel-statfs-io')
    return (int(result.kind)&0xffffffff,tuple(int(x) for x in result.fsid))


class NativeDirectoryIO:
    """Fixed libc read interfaces, no libc readdir prefetch and no syscall selector.

    ABI: Linux glibc getdents64/readlinkat, little-endian64bit selected X64 image.
    glibc2.39 getdents64 is a direct single syscall bounded by supplied count:
    https://github.com/bminor/glibc/blob/glibc-2.39/sysdeps/unix/sysv/linux/getdents64.c
    No installation, compiler, alternate endpoint or fallback implementation.
    """
    def __init__(self):
        require(sys.byteorder == 'little' and struct.calcsize('P') == 8, 'native-abi')
        lib = ctypes.CDLL(None, use_errno=True)
        self.lib = lib
        self.getdents = lib.getdents64
        self.getdents.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_size_t]
        self.getdents.restype = ctypes.c_ssize_t
        self.readlink = lib.readlinkat
        self.readlink.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t]
        self.readlink.restype = ctypes.c_ssize_t

    def directory(self, fd, count):
        require(type(count) is int and 0 < count <= 65536, 'native-buffer')
        buf = ctypes.create_string_buffer(count)
        result = self.getdents(fd, buf, count)  # Exactly one fixed bounded read call.
        if result < 0:
            raise OSError(ctypes.get_errno(), 'directory-read-refused')
        require(result <= count, 'native-return')
        return buf.raw[:result]

    def link(self, fd, name):
        buf = ctypes.create_string_buffer(4096)
        result = self.readlink(fd, os.fsencode(name), buf, 4096)
        if result < 0:
            raise OSError(ctypes.get_errno(), 'link-read-refused')
        require(result < 4096, 'link-size')  # Equal buffer size may be truncated.
        try:
            return buf.raw[:result].decode('ascii', 'strict')
        except UnicodeError:
            raise Refusal('link-encoding') from None


def directory_names(data, count_cap):
    """Decode only bounded Linux dirent64 records; never policy language/content."""
    found, offset = [], 0
    while offset < len(data):
        require(len(data)-offset >= 24, 'directory-header')
        length = struct.unpack_from('<H', data, offset+16)[0]
        require(length >= 24 and length % 8 == 0 and length <= len(data)-offset,
                'directory-record')
        field = data[offset+19:offset+length]
        end = field.find(b'\x00')
        require(0 < end <= 255, 'entry-name')
        name = os.fsdecode(field[:end])
        require('/' not in name, 'entry-name')
        if name not in ('.', '..'):
            require(len(found) < count_cap, 'directory-count')
            require(name not in found, 'directory-duplicate')
            found.append(name)
        offset += length
    return sorted(found)


class RealFS:
    """Descriptor-anchored no-follow traversal; only construction opens fixture/host root.

    root='/' is supplied ONLY by the future selected driver. Tests use private temp roots.
    Metadata accounting: charge256B BEFORE each stat. Directory records are read by
    a single bounded libc getdents64 call through the SAME ledger as file reads.
    Readlink reserves4096B before one fixed-buffer readlinkat; full debit on Err.
    All these debits share the input ledger. JSON output never prices input reads.
    Cleanup closes already-owned descriptors even after the deadline; performs no reads.
    """
    def __init__(self, ledger, root):
        self.b = ledger
        self.held_executables = {}
        self.policy_entry = None
        self.native = ledger.call(NativeDirectoryIO)
        self.fd = ledger.acquire(os.open, root, os.O_RDONLY | os.O_DIRECTORY |
                              os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            root_info = self._stat(self.fd)
            custody(root_info, True, site=1)
            self.directories = {'/': (self.fd, identity(root_info))}
        except BaseException:
            os.close(self.fd)
            raise

    def _stat(self, fd):
        self.b.charge(STAT_DEBIT)
        return self.b.call(os.fstat, fd)

    def _close_kernel_owned(self,fd):
        #Cleanup must run even after the deadline; never retry a numeric FD.
        try:
            os.close(fd)
        except OSError:
            raise Refusal('descriptor-cleanup') from None

    def _kernel_kind(self,fd):
        self.b.charge(STAT_DEBIT)  #256B BEFORE fixed120B metadata syscall, including Err.
        return self.b.call(kernel_fs_kind,self.native,fd)

    def _policy_link_info(self,parent):
        self.b.charge(STAT_DEBIT)
        return self.b.call(os.stat,'policy',dir_fd=parent,follow_symlinks=False)

    def _follow_fixed_policy(self,parent):
        #The ONLY proposed follow: literal policy under an already verified held
        #SECURITYFS parent. This is an explicit typed guard delta, not O_NOFOLLOW.
        try:
            return os.open('policy',os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC,dir_fd=parent)
        except OSError as exc:
            code=exc.errno if type(exc.errno) is int else None
            reason={errno.ENOTDIR:'policy-entry-not-directory',errno.ELOOP:'policy-entry-link-loop',
                    errno.EACCES:'policy-entry-access-denied',errno.EPERM:'policy-entry-not-permitted',
                    errno.ENOENT:'policy-entry-not-found'}.get(code,'policy-entry-other-io')
            raise Refusal(reason) from None

    def _acquire_policy_root(self,parent):
        parent_path=POLICY.rsplit('/',1)[0]
        require(parent_path in self.directories
                and self.directories[parent_path][0]==parent,'kernel-policy-root-state')
        before_parent=self._stat(parent)
        custody(before_parent,True, site=2)
        require(identity(before_parent)==self.directories[parent_path][1],
                'kernel-policy-parent-drift')
        parent_kind=self._kernel_kind(parent)
        require(parent_kind[0]==0x73636673,'kernel-policy-parent-fs')
        before_link=self._policy_link_info(parent)
        require(stat.S_ISLNK(before_link.st_mode),'kernel-policy-link-kind')
        #Symlink mode bits do not grant content mutation in this kernel filesystem;
        #custody comes from the held parent/filesystem/type and actual root owner.
        require(before_link.st_uid==0 and before_link.st_gid==0
                and before_link.st_dev==before_parent.st_dev,'kernel-policy-link-custody')
        fd=None
        retained=False
        try:
            fd=self.b.acquire(self._follow_fixed_policy,parent)
            target=self._stat(fd)
            custody(target,True, site=3)
            target_kind=self._kernel_kind(fd)
            require(target_kind[0]==0x5a3c69f0,'kernel-policy-target-fs')
            require(identity(self._policy_link_info(parent))==identity(before_link),
                    'kernel-policy-link-drift')
            require(identity(self._stat(parent))==identity(before_parent)
                    and self._kernel_kind(parent)==parent_kind,'kernel-policy-parent-drift')
            marker={'parent':parent,'link':identity(before_link),
                    'parent_identity':identity(before_parent),'parent_kind':parent_kind,
                    'target':identity(target),'target_kind':target_kind}
            result=(fd,marker)  #Allocate handoff before relinquishing FD cleanup.
            retained=True
            return result
        finally:
            if fd is not None and not retained:
                self._close_kernel_owned(fd)

    def _verify_policy_root(self,fd,before):
        marker=self.policy_entry
        require(type(marker) is dict and marker.get('target')==before,'kernel-policy-root-state')
        parent_path=POLICY.rsplit('/',1)[0]
        require(parent_path in self.directories
                and self.directories[parent_path][0]==marker['parent'],'kernel-policy-root-state')
        require(identity(self._stat(fd))==before
                and self._kernel_kind(fd)==marker['target_kind'],'kernel-policy-root-binding')
        check=None
        try:
            check,fresh=self._acquire_policy_root(marker['parent'])
            require(fresh==marker,'kernel-policy-root-binding')
            require(identity(self._stat(check))==before,'kernel-policy-root-binding')
        finally:
            if check is not None:
                self._close_kernel_owned(check)

    def _parent(self, path):
        parts = components(path)
        if len(parts) > 1:
            directory_allowed('/'+'/'.join(parts[:-1]))
        current, prefix = self.fd, ''
        for part in parts[:-1]:
            prefix += '/' + part
            if prefix not in self.directories:
                require(len(self.directories) < 512, 'directory-descriptor-cap')
                marker=None
                if prefix==POLICY:
                    child,marker=self._acquire_policy_root(current)
                else:
                    child = self.b.acquire(os.open, part, os.O_RDONLY | os.O_DIRECTORY |
                                           os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=current)
                try:
                    s = self._stat(child)
                    custody(s, True, site=4)
                    if marker is not None:
                        require(identity(s)==marker['target'],'kernel-policy-root-binding')
                        self.policy_entry=marker
                    self.directories[prefix] = (child, identity(s))
                except BaseException:
                    os.close(child)
                    raise
            current = self.directories[prefix][0]
        before = self._stat(current)
        custody(before, True, site=5)
        require(identity(before) == self.directories[prefix or '/'][1], 'directory-cache-drift')
        # Borrowed from this single-threaded collector's held cache, closed ONLY
        # by RealFS.close. No temporary duplicate or independent lifetime.
        return current, parts[-1]

    def info(self, path):
        parent, name = self._parent(path)
        self.b.charge(STAT_DEBIT)
        return self.b.call(os.stat, name, dir_fd=parent, follow_symlinks=False)

    def read(self, path, cap, hold=False, one_frame=False):
        require(path.rsplit('/', 1)[-1] != 'raw_data', 'raw-open-forbidden')
        if one_frame:
            require(path==POLICY+'/revision' and cap==64 and not hold, 'revision')
        parent, name = self._parent(path)
        fd = None
        retained = False
        try:
            fd = self.b.acquire(os.open, name, os.O_RDONLY | os.O_NOFOLLOW |
                             os.O_CLOEXEC | os.O_NONBLOCK, dir_fd=parent)
            before = self._stat(fd)
            custody(before, site=6)
            # Files such as securityfs/procfs can report st_size=0 with content.
            require(before.st_size <= cap, 'declared-size')
            reader=lambda count:os.read(fd,count)
            data = revision_frame(self.b,reader) if one_frame else self.b.read(reader,cap)
            after = self._stat(fd)
            custody(after, site=7)
            require(identity(before) == identity(after), 'held-file-drift')
            self.b.charge(STAT_DEBIT)
            named = self.b.call(os.stat, name, dir_fd=parent, follow_symlinks=False)
            require(identity(named) == identity(after), 'named-file-drift')
            if hold:
                require(len(self.held_executables) < 2 and path not in self.held_executables,
                        'held-executable-cap')
                self.held_executables[path] = (fd, identity(after))
                retained = True
            return data
        finally:
            if fd is not None and not retained:
                os.close(fd)

    def hold_read(self, path, cap):
        return self.read(path, cap, hold=True)

    def validate_executables(self):
        for path, (fd, before) in self.held_executables.items():
            held = self._stat(fd)
            custody(held, site=8)
            if identity(held) != before or identity(self.info(path)) != before:
                return False
        return len(self.held_executables) == 2

    def names(self, path, count_cap=1024):
        directory_allowed(path)  # BEFORE even resolving/opening its parent.
        parent, name = self._parent(path)
        fd = None
        try:
            fd = self.b.acquire(os.open, name, os.O_RDONLY | os.O_DIRECTORY |
                             os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
            before = self._stat(fd)
            custody(before, True, site=9)
            data = self.b.read(lambda count: self.native.directory(fd, count), 262144)
            found = directory_names(data, count_cap)
            require(identity(before) == identity(self._stat(fd)), 'directory-drift')
            self.b.charge(STAT_DEBIT)
            named = self.b.call(os.stat, name, dir_fd=parent, follow_symlinks=False)
            require(identity(named) == identity(before), 'named-directory-drift')
            return sorted(found)
        finally:
            if fd is not None:
                os.close(fd)

    def link(self, path):
        parent, name = self._parent(path)
        target = self.b.metadata(lambda: self.native.link(parent, name), 4096, 'link-size')
        # Links only allowed by metadata resolver, never followed by RealFS.read.
        return target

    def close(self):
        failed = False
        for fd, _ in list(self.directories.values()) + list(getattr(self,'held_executables',{}).values()):
            try:
                os.close(fd)
            except OSError:
                failed = True
        self.directories.clear()
        self.held_executables.clear()
        self.policy_entry=None
        require(not failed, 'descriptor-cleanup')

    def validate(self):
        # Every cached parent is checked BOTH by held FD and its held parent's name.
        # Thus a path renamed/replaced while children were read cannot pass output.
        for path in sorted(self.directories, key=lambda value: (value.count('/'), value)):
            fd, before = self.directories[path]
            held = self._stat(fd)
            custody(held, True, site=10)
            require(identity(held) == before, 'directory-cache-drift')
            if path==POLICY:
                self._verify_policy_root(fd,before)
            elif path != '/':
                parent, name = path.rsplit('/', 1)
                parent_fd = self.directories[parent or '/'][0]
                self.b.charge(STAT_DEBIT)
                named = self.b.call(os.stat, name, dir_fd=parent_fd, follow_symlinks=False)
                require(identity(named) == before, 'directory-cache-binding')


def text(fs, path, cap):
    try:
        return fs.read(path, cap).decode('utf-8', 'strict')
    except UnicodeError:
        raise Refusal('encoding') from None


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode()


def walk(fs, root, *, max_depth=16, max_files=2048, max_dirs=4096, links=()):
    stack = [(root, 0)]
    files, directories = [], 0
    while stack:
        current, depth = stack.pop()
        require(depth <= max_depth, 'tree-depth')
        directories += 1
        require(directories <= max_dirs, 'tree-directories')
        for name in fs.names(current):
            path = current + '/' + name
            s = fs.info(path)
            if name in links:
                # Reserved profile links are leaves. Check this BEFORE S_ISDIR.
                require(stat.S_ISLNK(s.st_mode) and s.st_uid == 0, 'profile-link-type')
                require(len(files) < max_files, 'tree-files')
                files.append(path)
            elif stat.S_ISDIR(s.st_mode):
                custody(s, True, site=11)
                stack.append((path, depth + 1))
            else:
                if stat.S_ISLNK(s.st_mode):
                    require(name in links and s.st_uid == 0, 'tree-link')
                else:
                    custody(s, site=12)
                require(len(files) < max_files, 'tree-files')
                files.append(path)
    return sorted(files)


def inventory(fs):
    paths = walk(fs, POLICY + '/profiles', links=PROFILE_LINKS)
    groups = {}
    for path in paths:
        parent, key = path.rsplit('/', 1)
        groups.setdefault(parent, set()).add(key)
    rows = []
    for parent, keys in sorted(groups.items()):
        if {'name', 'mode', 'attach'} <= keys:
            require('sha256' in keys, 'profile-sha-missing')
            values = {key: text(fs, parent + '/' + key, 4096)
                      for key in ('name', 'mode', 'attach', 'sha256')}
            require(HEX64.fullmatch(values['sha256'].strip()), 'profile-sha')
            rows.append((parent, values))
    require(rows and len(rows) <= 2048, 'profile-count')
    ordered = sorted((v for _, v in rows), key=lambda v: json.dumps(v, sort_keys=True))
    return rows, sha(canonical(ordered))


def raw_link(fs, directory, key):
    target = fs.link(directory + '/' + key)
    require(not target.startswith('/') and '\x00' not in target, 'raw-link')
    path = os.path.normpath(directory + '/' + target)
    prefix = POLICY + '/raw_data/'
    require(path.startswith(prefix) and len(path) <= 4096, 'raw-link-escape')
    # Every intermediate component is opened NOFOLLOW and root-custody checked.
    return path


def raw_sizes(fs, rows, expected):
    found, selected_rows = {}, 0
    for directory, values in rows:
        if values['attach'].strip() != '<unknown>':
            continue
        selected_rows += 1
        raw = raw_link(fs, directory, 'raw_data')
        digest_path = raw_link(fs, directory, 'raw_sha256')
        abi_path = raw_link(fs, directory, 'raw_abi')
        parent = raw.rsplit('/', 1)[0]
        require(raw == parent + '/raw_data' and digest_path == parent + '/sha256'
                and abi_path == parent + '/abi', 'raw-link-layout')
        digest = text(fs, digest_path, 128).strip()
        abi = text(fs, abi_path, 64).strip()
        require(HEX64.fullmatch(digest) and digest in expected, 'raw-selector')
        require(abi == 'v5', 'raw-abi')
        before = fs.info(raw)  # NEVER open raw_data, not even O_PATH.
        custody(before, site=13)
        require(before.st_size > 0, 'raw-size-unavailable')
        require(text(fs, digest_path, 128).strip() == digest
                and text(fs, abi_path, 64).strip() == abi, 'raw-metadata-drift')
        require(identity(before) == identity(fs.info(raw)), 'raw-endpoint-drift')
        record = {'selector': digest, 'abi': abi, 'bytes': before.st_size,
                  'device': before.st_dev, 'inode': before.st_ino,
                  'raw_data_opened': False}
        require(digest not in found or found[digest] == record, 'raw-alias-drift')
        found[digest] = record
    require(selected_rows == 100 and set(found) == set(expected), 'raw-selector-set')
    return [found[key] for key in sorted(found)]


def sources(fs, manifest):
    result = []
    for entry in manifest:
        data = fs.read(entry['path'], entry['bytes'])
        digest = sha(data)
        require(len(data) == entry['bytes'] and digest == entry['sha256'], 'source-hash')
        # Never export content, names from content, or mismatching source bytes.
        result.append({'path': entry['path'], 'bytes': len(data), 'sha256': digest})
    return result


def executables(fs, manifest):
    result = []
    for entry in manifest:
        data = fs.hold_read(entry['path'], entry['bytes'])
        digest = sha(data)
        require(len(data) == entry['bytes'] and digest == entry['sha256'], 'executable-hash')
        result.append({'path':entry['path'],'bytes':len(data),'sha256':digest})
    return result


def conditionals(fs, manifest, public):
    rows = []
    for entry in manifest:
        path = '/' + entry['target']
        try:
            s = fs.info(path)
        except FileNotFoundError:
            require(entry['optional'], 'mandatory-absent')
            rows.append({'path': path, 'state': 'absent'})
            continue
        if stat.S_ISDIR(s.st_mode):
            custody(s, True, site=14)
            require(entry['evidence_state'] == 'candidate_directory_contents_only',
                    'unreviewed-directory')
            expected = {x.rsplit('/', 1)[-1] for x in entry['known_public_children']}
            require(set(fs.names(path)) == expected, 'directory-membership')
            for name in sorted(expected):
                child = path + '/' + name
                require(child in public, 'directory-child-unreviewed')
                sources(fs, [public[child]])
            rows.append({'path': path, 'state': 'reviewed-directory', 'children': len(expected)})
        else:
            custody(s, site=15)
            if s.st_size:
                require(path in public, 'private-local-nonempty')
                sources(fs, [public[path]])
            else:
                # Read/probe zero bytes under same ledger: a zero stat is not EOF proof.
                require(fs.read(path, 0) == b'', 'empty-local-drift')
            rows.append({'path': path, 'state': 'reviewed-file' if s.st_size else 'empty'})
    return rows


def features(fs, spec):
    paths = walk(fs, spec['absolute_root'], max_depth=spec['depth_cap'],
                 max_files=spec['file_count_cap'], max_dirs=256)
    require(paths, 'features-empty')
    result, size = [], 0
    for path in paths:
        relative = path[len(spec['absolute_root'])+1:]
        require(all(re.fullmatch(r'[A-Za-z0-9_.-]{1,64}', p) for p in relative.split('/')),
                'feature-path')
        data = fs.read(path, min(spec['per_file_content_cap'], spec['aggregate_content_cap']-size))
        size += len(data)
        require(size <= spec['aggregate_content_cap'], 'feature-total')
        # Only fixed-root kernel feature ABI ASCII fields, never policy source.
        require(re.fullmatch(rb'[A-Za-z0-9_\t \r\n.,:+/()=-]*', data), 'feature-value')
        result.append({'path': path, 'bytes': len(data), 'sha256': sha(data),
                       'abi_value': data.decode('ascii')})
    return result


def revision(fs):
    try:
        value=fs.read(POLICY+'/revision',64,one_frame=True).decode('utf-8','strict')
    except UnicodeError:
        raise Refusal('encoding') from None
    require(re.fullmatch(r'[0-9]+\n', value), 'revision')
    return value


def collect(fs, budget, spec, binding):
    budget.check()
    first = revision(fs)
    rows, policy_hash = inventory(fs)
    require(policy_hash == spec['policy_inventory_sha256'], 'policy-inventory-drift')
    sizes = raw_sizes(fs, rows, spec['raw_selectors'])
    public = {x['path']: x for x in spec['sources']}
    source_result = sources(fs, spec['sources'])
    config = sources(fs, [spec['config']])
    elf = executables(fs, spec['executables'])
    conditional_result = conditionals(fs, spec['conditionals'], public)
    feature_result = features(fs, spec['features'])
    # Re-read all predicates: no independent sub-ledgers, no timer reset on Err.
    require(sources(fs, spec['sources']) == source_result, 'source-drift')
    require(sources(fs, [spec['config']]) == config, 'config-drift')
    require(conditionals(fs, spec['conditionals'], public) == conditional_result,
            'conditional-drift')
    require(features(fs, spec['features']) == feature_result, 'features-drift')
    end_rows, end_hash = inventory(fs)
    require(end_hash == policy_hash and raw_sizes(fs, end_rows, spec['raw_selectors']) == sizes
            and revision(fs) == first, 'policy-revision-drift')
    fs.validate()
    require(fs.validate_executables(), 'executable-drift')
    budget.check()
    return {'schema': 'tos_public_source_identity_witness_v1',
            'outcome': 'diagnostic-only', 'diagnostic_exit_code': 79,
            'binding': binding, 'revision': first.strip(), 'policy_inventory_sha256': policy_hash,
            'sources': source_result, 'parser_config': config, 'executables': elf,
            'conditional_inputs': conditional_result, 'feature_hashes': feature_result,
            'raw_endpoint_sizes': sizes, 'feature_abi_ascii_exported': True,
            'source_contents_exported': False, 'raw_data_opened': False,
            'compiled_stream_matches': 0, 'source_closure_proven': False,
            'attachment_admission': False, 'ticket_issued': False,
            'namespace_attempted': False, 'policy_changed': False,
            'operational_status': 'NOTRUN',
            'accounting': {'input_bytes': budget.used, 'input_cap': INPUT_CAP,
                           'metadata_debits_included': True, 'operations_before_output': budget.ops,
                           'deadline_seconds': SECONDS, 'wire_cap': OUTPUT_CAP}}


def encode(budget, report):
    wire = bytearray()
    for part in json.JSONEncoder(sort_keys=True, ensure_ascii=True).iterencode(report):
        budget.check()
        data = part.encode('ascii')
        require(len(data) < OUTPUT_CAP-len(wire), 'output-budget')
        wire.extend(data)
    wire.extend(b'\n')
    budget.check()
    return bytes(wire)


def emit(budget, wire, writer=os.write):
    require(len(wire) <= OUTPUT_CAP, 'output-budget')
    pin = ('TOS_SOURCE_WITNESS_SHA256 ' + sha(wire) + ' bytes=' + str(len(wire)) + '\n').encode()
    require(len(wire) + len(pin) + STATUS_RESERVE <= OUTPUT_CAP, 'total-output-budget')
    for fd, data in ((1, wire), (2, pin)):
        offset = 0
        while offset < len(data):
            count = budget.call(writer, fd, memoryview(data)[offset:])
            require(type(count) is int and 0 < count <= len(data)-offset, 'writer-contract')
            offset += count


def run_selected(spec, binding):
    """Only immutable workflow driver invokes this after its external identity checks."""
    budget = Ledger()  # BEFORE first host read, including RealFS constructor.
    fs = None
    changed = False
    old_handler = None
    result = 100
    cleanup_mask = 0
    try:
        require(binding.get('platform_binding') == 'coherent' and os.geteuid() == 0
                and sys.flags.isolated and sys.flags.no_site, 'selected-context')
        old_handler = budget.call(signal.getsignal, signal.SIGALRM)
        require(budget.call(signal.getitimer, signal.ITIMER_REAL) == (0.0, 0.0), 'existing-timer')
        def expired(signum, frame):
            raise Refusal('deadline')
        # Mark the attempted handler transition first, so an exception immediately
        # after signal.signal cannot skip restoration of our saved handler.
        changed = True
        signal.signal(signal.SIGALRM, expired)
        signal.setitimer(signal.ITIMER_REAL, max(0.000001, budget.deadline-budget.clock()))
        fs = RealFS(budget, '/')
        report = collect(fs, budget, spec, binding)
        report['accounting']['status_reserve_bytes'] = STATUS_RESERVE
        emit(budget, encode(budget, report))
        result = 79
    except BaseException as exc:
        #Only a public fixed code leaves this process through its exit status.
        #No post-failure read, JSON, exception text, private value or error write.
        result = refusal_exit(exc, Refusal)
    finally:
        try:
            if fs is not None:
                fs.close()
        except BaseException:
            cleanup_mask |= 1  # Preserve all later restoration attempts.
        finally:
            if changed:
                try:
                    signal.setitimer(signal.ITIMER_REAL, 0)
                except BaseException:
                    cleanup_mask |= 2
                finally:
                    try:
                        signal.signal(signal.SIGALRM, old_handler)
                    except BaseException:
                        cleanup_mask |= 4
    return cleanup_exit(result, cleanup_mask)


if __name__ == '__main__':
    raise SystemExit('Inert source: direct checkout execution disabled.')
