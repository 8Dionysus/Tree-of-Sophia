const live=new URL(location.href).searchParams.get('live')==='1';
await (live?import('../src/research.mjs'):import('./demo.mjs'));
