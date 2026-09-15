import {afterEach, expect, test, vi} from 'vitest';
import {scheduleExpiry} from './expiry.mjs';

afterEach(()=>vi.useRealTimers());

test('long-lived reading expires at the owner deadline, not at the timer ceiling',()=>{
  vi.useFakeTimers();vi.setSystemTime(new Date('2026-01-01T00:00:00Z'));
  const start=Date.now(),delay=2147483647,expire=vi.fn();
  scheduleExpiry(new Date(start+delay+10000).toISOString(),expire);
  vi.advanceTimersByTime(delay);expect(expire).not.toHaveBeenCalled();
  vi.advanceTimersByTime(9999);expect(expire).not.toHaveBeenCalled();
  vi.advanceTimersByTime(1);expect(expire).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(delay);expect(expire).toHaveBeenCalledTimes(1);
});

test('deadline recheck tolerates a backward clock change and cancellation',()=>{
  vi.useFakeTimers();vi.setSystemTime(new Date('2026-01-01T00:00:00Z'));
  const start=Date.now(),expire=vi.fn();
  const cancel=scheduleExpiry(new Date(start+10000).toISOString(),expire);
  vi.setSystemTime(start-10000);vi.advanceTimersByTime(10000);
  expect(expire).not.toHaveBeenCalled();cancel();
  vi.advanceTimersByTime(20000);expect(expire).not.toHaveBeenCalled();
  scheduleExpiry(new Date(Date.now()-1).toISOString(),expire);
  expect(expire).toHaveBeenCalledTimes(1);
});
