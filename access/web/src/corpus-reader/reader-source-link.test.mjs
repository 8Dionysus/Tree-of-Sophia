import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect, test, vi} from 'vitest';
import {Window} from 'happy-dom';
import {createFixtureProvider} from './fixture-provider.mjs';
import {mountCorpusReader} from './reader.mjs';

test.each([true, false])('selected passage shows the correct source link (unit-specific: %s)', async unitSpecific => {
  const browser = new Window({url: 'https://example.test'});
  vi.stubGlobal('window', browser); vi.stubGlobal('document', browser.document);
  vi.stubGlobal('navigator', browser.navigator); vi.stubGlobal('NodeFilter', browser.NodeFilter);
  const writeText = vi.fn(async () => {});
  Object.defineProperty(browser.navigator, 'clipboard', {value: {writeText}, configurable: true});
  const sourceUrl = unitSpecific ? 'https://www.gutenberg.org/files/42933/42933-h/42933-h.htm#i_1' : 'https://example.test/volume-1';
  const fixture = createFixtureProvider({documentCount: 1, unitCount: 1, languages: ['en']});
  const provider = {...fixture, async document(args) {
    const result = await fixture.document(args);
    return {...result, versions: result.versions.map(version => ({...version,
      sourceUrl: 'https://example.test/volume-1', additionalSourceUrls: ['https://example.test/volume-2']}))};
  }, async window(args) {
    const page = await fixture.window(args);
    return {...page, units: page.units.map(unit => unitSpecific ? {...unit, sourceUrl} : unit)};
  }};
  const host = document.createElement('div'); document.body.append(host);
  const reader = mountCorpusReader({host, provider, locale: () => 'en'});
  try {
    await reader.open();
    await vi.waitFor(() => expect(host.querySelector('.cr-unit-marker')).not.toBeNull());
    host.querySelector('.cr-unit-marker').click();
    await vi.waitFor(() => expect(host.querySelector(`a[href="${sourceUrl}"]`)).not.toBeNull());
    host.querySelector('.cr-copy').click();
    await vi.waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(writeText.mock.calls[0][0]).toContain(sourceUrl);
    [...host.querySelectorAll('.cr-tab')].find(tab => tab.textContent === 'Source').click();
    await vi.waitFor(() => expect(host.querySelector('a[href="https://example.test/volume-2"]')).not.toBeNull());
  } finally { await reader.destroy(); host.remove(); vi.unstubAllGlobals(); await browser.happyDOM.close(); }
});
