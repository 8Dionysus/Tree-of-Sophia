const MAX_TIMER_DELAY = 2147483647;

// Timers have a signed 32-bit delay, while owner deadlines may be years away.
// Every wake checks the actual deadline, including after a suspended tab wakes.
export function scheduleExpiry(expiresAt, expire, {now = Date.now, setTimer = setTimeout, clearTimer = clearTimeout} = {}) {
  const deadline = Date.parse(expiresAt);
  if (!Number.isFinite(deadline)) throw new TypeError('Invalid reading expiry.');
  let timer = null, cancelled = false;
  const check = () => {
    if (cancelled) return;
    const remaining = deadline - now();
    if (remaining <= 0) { cancelled = true; expire(); }
    else timer = setTimer(check, Math.min(remaining, MAX_TIMER_DELAY));
  };
  check();
  return () => { cancelled = true; if (timer !== null) clearTimer(timer); };
}
