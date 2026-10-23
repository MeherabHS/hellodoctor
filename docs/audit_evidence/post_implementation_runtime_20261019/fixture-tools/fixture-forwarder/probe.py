import json
import socket
import struct
import time


def read_exact(conn, count):
    data = bytearray()
    while len(data) < count:
        chunk = conn.recv(count - len(data))
        if not chunk:
            raise RuntimeError("Fixture connection closed")
        data.extend(chunk)
    return bytes(data)


def message(conn):
    tag = read_exact(conn, 1)
    length = struct.unpack("!I", read_exact(conn, 4))[0]
    if length < 4 or length > 1048576:
        raise RuntimeError("Unexpected bounded fixture message size")
    body = read_exact(conn, length - 4)
    if tag == b"E":
        raise RuntimeError("PostgreSQL fixture returned an error")
    return tag, body


def connect_pg(application):
    conn = socket.create_connection(("127.0.0.1", 15432), timeout=3)
    conn.settimeout(3)
    params = (
        b"user\0postgres\0database\0postgres\0application_name\0"
        + application.encode("ascii")
        + b"\0\0"
    )
    conn.sendall(struct.pack("!II", 8 + len(params), 196608) + params)
    while True:
        tag, _ = message(conn)
        if tag == b"Z":
            return conn


def query_scalar(conn, sql):
    body = sql.encode("ascii") + b"\0"
    conn.sendall(b"Q" + struct.pack("!I", 4 + len(body)) + body)
    result = None
    while True:
        tag, body = message(conn)
        if tag == b"D":
            fields = struct.unpack("!H", body[:2])[0]
            if fields != 1:
                raise RuntimeError("Expected one scalar fixture column")
            length = struct.unpack("!i", body[2:6])[0]
            result = body[6 : 6 + length].decode("ascii")
        if tag == b"Z":
            return result


started = time.monotonic()
connections = []
try:
    for _ in range(8):
        connections.append(connect_pg("hd-acceptance-probe"))
    observer = connect_pg("hd-acceptance-observer")
    try:
        observed = int(
            query_scalar(
                observer,
                "SELECT count(*) FROM pg_stat_activity WHERE application_name='hd-acceptance-probe'",
            )
        )
        if observed != 8:
            raise RuntimeError("Eight simultaneous fixture backends not observed")
        database = query_scalar(observer, "SELECT current_database()")
    finally:
        observer.close()
    with socket.create_connection(("127.0.0.1", 16379), timeout=3) as redis:
        redis.settimeout(3)
        redis.sendall(b"*1\r\n$4\r\nPING\r\n")
        pong = read_exact(redis, 7)
        if pong != b"+PONG\r\n":
            raise RuntimeError("Fixture Redis PING failed")
    print(
        json.dumps(
            {
                "verdict": "PASS",
                "pg_loopback": "127.0.0.1:15432",
                "redis_loopback": "127.0.0.1:16379",
                "pg_simultaneous_connections_observed": observed,
                "pg_database": database,
                "redis_ping": "PONG",
                "duration_seconds": time.monotonic() - started,
            }
        )
    )
finally:
    for connection in connections:
        connection.close()
