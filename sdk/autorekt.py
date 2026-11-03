"""autorekt monitor SDK (Python standard library only).

Monitors are event-driven actors in the style of Nautilus Trader / QuantConnect LEAN:

    from autorekt import Monitor

    class BasisWatch(Monitor):
        def on_start(self):
            self.subscribe_bars("okx:BTC/USDT:USDT", "1m")
            self.schedule(every="1h", fn=self.report)

        def on_bar(self, bar):                      # closed bars only
            perp = self.history("okx:BTC/USDT:USDT", "1m", 1)[-1]["close"]
            spot = self.history("okx:BTC/USDT", "1m", 1)[-1]["close"]
            basis = perp / spot - 1
            self.record("basis.btc", basis)          # -> series local:basis.btc
            if abs(basis) > 0.002:
                self.alert(f"BTC basis {basis:.3%}", severity=2)

        def report(self):
            self.log("hourly check")

    if __name__ == "__main__":
        BasisWatch().run()

history() returns a list of bar dicts with the columns timestamp, open, high, low, close,
volume, closed (pass it to pandas.DataFrame if you want a frame).

Drop the file into ~/.autorekt/monitors/ and the daemon runs and supervises it
(restarts on crash, logs to ~/.autorekt/logs/monitor-<name>.log). The SDK finds the daemon
through $AUTOREKT_HOME/daemon.json (default ~/.autorekt).
"""

import json
import os
import sys
import time
import traceback
import urllib.error
import urllib.request

__all__ = ["Monitor", "Bar", "api"]

_TF = {"m": 60, "h": 3600, "d": 86400, "w": 604800, "M": 2592000}


def tf_seconds(tf):
    n, u = tf[:-1] or "1", tf[-1]
    return int(n) * _TF[u]


def _conn():
    home = os.environ.get("AUTOREKT_HOME", os.path.expanduser("~/.autorekt"))
    path = os.path.join(home, "daemon.json")
    with open(path) as f:
        d = json.load(f)
    return "http://127.0.0.1:%d" % d["port"], d["token"]


def api(path, body):
    """POST to the daemon's local API and return the decoded JSON."""
    url, tok = _conn()
    req = urllib.request.Request(
        url + "/api/" + path,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer " + tok},
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        raise RuntimeError("autorekt %s: HTTP %d: %s" % (path, e.code, e.read().decode())) from None


class Bar:
    __slots__ = ("id", "tf", "timestamp", "open", "high", "low", "close", "volume", "closed")

    def __init__(self, id, tf, d):
        self.id, self.tf = id, tf
        for k in ("timestamp", "open", "high", "low", "close", "volume", "closed"):
            setattr(self, k, d[k])

    def __repr__(self):
        return "Bar(%s %s t=%s o=%s h=%s l=%s c=%s v=%s)" % (
            self.id, self.tf, self.timestamp, self.open, self.high, self.low, self.close, self.volume)


class Monitor:
    """Subclass and override on_start / on_bar / on_stop. Call .run()."""

    poll_floor = 5.0  # seconds between polls of a subscription, at least

    def __init__(self):
        self.name = os.environ.get("AUTOREKT_MONITOR", type(self).__name__)
        self._subs = {}    # (id, tf) -> last closed timestamp
        self._timers = []  # [interval_s, next_due, fn]
        self._running = False

    # ---- lifecycle hooks
    def on_start(self):
        pass

    def on_bar(self, bar):
        pass

    def on_stop(self):
        pass

    # ---- subscriptions and timers
    def subscribe_bars(self, id, tf="1m"):
        self._subs[(id, tf)] = None

    def schedule(self, every, fn):
        s = tf_seconds(every)
        self._timers.append([s, time.time() + s, fn])

    # ---- data
    def history(self, id, tf="1h", n=200):
        """Last n bars of an id or expression, oldest first, as a list of dicts."""
        return api("ohlcv", {"id": id, "tf": tf, "limit": int(n)})

    def eval(self, expr, tf="1h", n=100):
        """Evaluate an autorekt expression; returns a list of (timestamp_ms, value)."""
        r = api("eval", {"expr": expr, "tf": tf, "limit": int(n)})
        return list(zip(r["timestamp"], r["value"]))

    # ---- outputs
    def record(self, series, value, timestamp=None):
        """Append a point to a local series (readable as local:<series>)."""
        ts = int(timestamp if timestamp is not None else time.time() * 1000)
        api("record", {"series": series, "points": [{"timestamp": ts, "value": float(value)}]})

    def alert(self, title, body="", severity=2, symbols=None, hook=None):
        """Emit an alert event (wakes `autorekt wait`, shows in `diff`, runs the hook if given)."""
        return api("event", {"title": title, "body": body, "severity": int(severity), "kind": "alert",
                             "source": "monitor/" + self.name, "symbols": symbols or [], "hook": hook})

    def emit_event(self, kind, title, body="", severity=0):
        return api("event", {"title": title, "body": body, "severity": int(severity), "kind": kind,
                             "source": "monitor/" + self.name})

    def log(self, *args):
        print(time.strftime("%Y-%m-%d %H:%M:%S"), self.name, *args, flush=True)

    # ---- loop
    def _poll(self, key):
        id, tf = key
        bars = api("ohlcv", {"id": id, "tf": tf, "limit": 3})
        closed = [b for b in bars if b["closed"]]
        last = self._subs[key]
        new = [b for b in closed if last is None or b["timestamp"] > last]
        if last is None:
            new = new[-1:]  # on start, deliver only the latest closed bar
        for b in new:
            self._subs[key] = b["timestamp"]
            self.on_bar(Bar(id, tf, b))

    def run(self):
        self._running = True
        self.log("starting")
        self.on_start()
        next_poll = {k: 0.0 for k in self._subs}
        try:
            while self._running:
                now = time.time()
                for key in list(self._subs):
                    if now >= next_poll.get(key, 0):
                        try:
                            self._poll(key)
                        except Exception:
                            traceback.print_exc()
                        next_poll[key] = now + max(self.poll_floor, tf_seconds(key[1]) / 6)
                for t in self._timers:
                    if now >= t[1]:
                        t[1] = now + t[0]
                        try:
                            t[2]()
                        except Exception:
                            traceback.print_exc()
                time.sleep(1.0)
        except KeyboardInterrupt:
            pass
        finally:
            self.on_stop()
            self.log("stopped")

    def stop(self):
        self._running = False


if __name__ == "__main__":
    print(__doc__)
    sys.exit(0)
