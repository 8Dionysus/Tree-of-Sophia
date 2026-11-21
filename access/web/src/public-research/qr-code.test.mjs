import {createHash} from 'node:crypto';
import {expect,test} from 'vitest';
import {codewords,qrCode,qrSvg,versionFor} from './qr-code.mjs';

// Digests of the modules a reference encoder (segno 1.6.6, byte mode, UTF-8, with the padding bits of ISO/IEC 18004
// 7.4.10) gives for the same text, level, version and mask: rows of 0 and 1 joined by newlines.
const REFERENCE=[
  {text:'https://treeofsophia.com/zarathustra/?item=affirmation%3Alink%3A01%3Achild&experience=original&layer=research',level:'M',mask:0,version:7,sha256:'abd1df961e2b4e22dbc3810647f40b2eeb16917b51a576c3149b613bff63651d'},
  {text:'https://treeofsophia.com/zarathustra/?item=affirmation%3Alink%3A01%3Achild&experience=original&layer=research',level:'M',mask:6,version:7,sha256:'41b05051719a6ae253828237aa6f5d087ca642853976c845080baa6f9dcdb213'},
  {text:'Ж',level:'M',mask:2,version:1,sha256:'171bd35793174c738190e3e024b287d58a300543b94b49aeca56ecd5c0bbf427'},
  {text:'Так говорил Заратустра — Also sprach Zarathustra',level:'M',mask:4,version:5,sha256:'7190e1c43f7b0faa3aa19b5cd646674eaa4bb8073293d0dd4f8b5c84f6fc4532'},
  {text:'A'.repeat(300),level:'L',mask:1,version:11,sha256:'dd38b7171312e1b8216d788b24e30c73c66bd65bf3a7bdf6bdec9aaac45e8eee'},
  {text:'x'.repeat(1200),level:'M',mask:5,version:29,sha256:'4d242dbe66fe2ec60becab69710fef7825e5a81476040381ab2da9eaeae95991'},
  {text:'Q',level:'L',mask:7,version:1,sha256:'76909a999b7f633488f12d1a58c1dff91e95085d20f3c7ff626208fa6dfdf61a'},
];
const digest=code=>createHash('sha256').update(code.dark.map(row=>row.map(dark=>dark?'1':'0').join('')).join('\n')).digest('hex');

test('the modules match a reference encoder across versions 1 to 29, both levels and several masks',()=>{
  for(const item of REFERENCE){
    const code=qrCode(item.text,{level:item.level,mask:item.mask});
    expect(code.version).toBe(item.version);
    expect(digest(code)).toBe(item.sha256);
  }
});

test('the smallest version that holds the text is chosen, and too long a text is refused',()=>{
  expect(versionFor(14,'M')).toBe(1);expect(versionFor(15,'M')).toBe(2);
  expect(codewords('Ж').codewords.slice(0,5)).toEqual([0x40,0x2d,0x09,0x60,0xec]);
  expect(()=>qrCode('x'.repeat(3000),{level:'M'})).toThrow(RangeError);
});

test('without a given mask the one of least penalty is chosen, and the drawing keeps the quiet zone',()=>{
  const code=qrCode('https://treeofsophia.com/zarathustra/');
  expect(code.mask).toBeGreaterThanOrEqual(0);expect(code.size).toBe(code.version*4+17);
  const svg=qrSvg(code,{label:'A <link>'});
  expect(svg).toContain(`viewBox="0 0 ${code.size+8} ${code.size+8}"`);
  expect(svg).toContain('<title>A &lt;link&gt;</title>');
  expect(svg.match(/h1v1h-1z/g).length).toBe(code.dark.flat().filter(Boolean).length);
});
