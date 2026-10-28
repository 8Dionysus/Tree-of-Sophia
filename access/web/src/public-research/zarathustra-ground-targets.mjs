/** Navigation to reviewed entry points in the selected public versions.
 * Chapter targets are labelled as chapters. Paragraph targets were read in
 * each version independently; they do not assert translation alignment.
 * Kept outside the graph library so saved graph fingerprints stay stable. */
export const ZARATHUSTRA_DOCUMENT_ID = "tos.work.friedrich-nietzsche.also-sprach-zarathustra";
export const GROUND_VERSIONS = {
  "de": {
    "id": "gutenberg-7205-de",
    "revision": "4f71adfeeb39e64055acc2fc3f58964cf39f03d27bf2264d25313bc45d8658ec"
  },
  "en": {
    "id": "gutenberg-1998-common-en",
    "revision": "0854ebabe235d46c8e5e099418c5189b1df0848898c552009b70d924b9065d05"
  }
};
export const CHAPTER_GROUNDS = {
  "z-vision": {
    "title": {
      "ru": "О видении и загадке",
      "en": "The Vision and the Enigma"
    },
    "units": {
      "de": "tos.text-unit.sid-743ae4e5b8c74dcc8be64dfb5911f62b",
      "en": "tos.text-unit.sid-d65c182a5b3d4f28ab72f1426b1fb83a"
    }
  },
  "z-convalescent": {
    "title": {
      "ru": "Выздоравливающий",
      "en": "The Convalescent"
    },
    "units": {
      "de": "tos.text-unit.sid-003e79ab8bee489dbc1efaab7ffd4b02",
      "en": "tos.text-unit.sid-725ea2c7de004f538a1bd303089260be"
    }
  },
  "z-redemption": {
    "title": {
      "ru": "Об избавлении",
      "en": "Redemption"
    },
    "units": {
      "de": "tos.text-unit.sid-87cd56156c634bac830a5314a25cab47",
      "en": "tos.text-unit.sid-48dcac0bb0bd49389cb7b2f206b706b6"
    }
  },
  "z-prologue": {
    "title": {
      "ru": "Предисловие Заратустры",
      "en": "Zarathustra’s Prologue"
    },
    "units": {
      "de": "tos.text-unit.sid-625c9ed754f642c196d9428406076eee",
      "en": "tos.text-unit.sid-7810450af24e4a579995664d49fa5bc3"
    }
  },
  "z-metamorphoses": {
    "title": {
      "ru": "О трёх превращениях",
      "en": "The Three Metamorphoses"
    },
    "units": {
      "de": "tos.text-unit.sid-d3a7f2db839e4676aa523dcba8398c59",
      "en": "tos.text-unit.sid-6a954032e4aa44a79cfde098abf249f4"
    }
  },
  "z-pitiful": {
    "title": {
      "ru": "О сострадательных",
      "en": "The Pitiful"
    },
    "units": {
      "de": "tos.text-unit.sid-9836a3e891164b75860d369bb36c1d30",
      "en": "tos.text-unit.sid-e3a03bf37cf746f1b280f51a7d2383b5"
    }
  },
  "z-sign": {
    "title": {
      "ru": "Знамение",
      "en": "The Sign"
    },
    "units": {
      "de": "tos.text-unit.sid-b5b911ed418145d28ea0662e134094ff",
      "en": "tos.text-unit.sid-5fae895dc764467980e5c27569af6336"
    }
  },
  "z-seven-seals": {
    "title": {
      "ru": "Семь печатей",
      "en": "The Seven Seals"
    },
    "units": {
      "de": "tos.text-unit.sid-06ad3f6ff0154946aba2f52258b13be9",
      "en": "tos.text-unit.sid-e821b8491ead49e4bf901d0acc20c782"
    }
  },
  "z-happy-isles": {
    "title": {
      "ru": "На блаженных островах",
      "en": "In the Happy Isles"
    },
    "units": {
      "de": "tos.text-unit.sid-14a94dbf839f4c26bf69ab18e391b590",
      "en": "tos.text-unit.sid-8a163d181e674f23bd6dc7743460a0ac"
    }
  },
  "z-drunken-song": {
    "title": {
      "ru": "Песнь опьянения",
      "en": "The Drunken Song"
    },
    "units": {
      "de": "tos.text-unit.sid-77da21723caa41a9a0396a6333e77b7f",
      "en": "tos.text-unit.sid-f62ab86b493a4a879121187aba605517"
    }
  }
};
export const NODE_GROUND_PASSAGES = {
  "demo:gateway": {
    "z-vision": {
      "de": "tos.text-unit.sid-9a7e0d9b40004d93935f6141982e678e",
      "en": "tos.text-unit.sid-327b4400bd0e447ba87e7c9159ba06f3"
    }
  },
  "demo:two-paths": {
    "z-vision": {
      "de": "tos.text-unit.sid-9a7e0d9b40004d93935f6141982e678e",
      "en": "tos.text-unit.sid-327b4400bd0e447ba87e7c9159ba06f3"
    }
  },
  "moment": {
    "z-vision": {
      "de": "tos.text-unit.sid-0bbb683276d04438a2ed2d82629801a1",
      "en": "tos.text-unit.sid-9b96d50ad307440fb8f8629c0ba79bc1"
    }
  },
  "demo:bite": {
    "z-vision": {
      "de": "tos.text-unit.sid-ae92a7183e344bb0998780e8dca0be10",
      "en": "tos.text-unit.sid-717a4cfe1b8e4c1ba3b01a44983b1caa"
    },
    "z-convalescent": {
      "de": "tos.text-unit.sid-f19d0a3e095c4152bbf4a3b83b4317fc",
      "en": "tos.text-unit.sid-23f8217407454b198dbc0f0300af8683"
    }
  },
  "demo:shepherd": {
    "z-vision": {
      "de": "tos.text-unit.sid-3bfa9f07f9614a4aa567c250edb713cd",
      "en": "tos.text-unit.sid-87fbf783426f4567a6a7f21c07326e45"
    }
  },
  "demo:laughter": {
    "z-vision": {
      "de": "tos.text-unit.sid-bbbb35366b7b47a69a1c6244e968c510",
      "en": "tos.text-unit.sid-763be4941b0149acbd66ebef21590667"
    }
  },
  "demo:animals": {
    "z-convalescent": {
      "de": "tos.text-unit.sid-dd3d56723f434f52b07b4139bc1b35aa",
      "en": "tos.text-unit.sid-a10667648407429880dbdaa450b3e310"
    }
  },
  "all-things": {
    "z-convalescent": {
      "de": "tos.text-unit.sid-dd3d56723f434f52b07b4139bc1b35aa",
      "en": "tos.text-unit.sid-a10667648407429880dbdaa450b3e310"
    }
  },
  "same-life": {
    "z-convalescent": {
      "de": "tos.text-unit.sid-9b2a502ab9134c5996852ab4147fd4a5",
      "en": "tos.text-unit.sid-91a33c66d5014c579190a50f7a20c900"
    }
  },
  "demo:identity": {
    "z-convalescent": {
      "de": "tos.text-unit.sid-9b2a502ab9134c5996852ab4147fd4a5",
      "en": "tos.text-unit.sid-91a33c66d5014c579190a50f7a20c900"
    }
  },
  "demo:recurrence": {
    "z-vision": {
      "de": "tos.text-unit.sid-0bbb683276d04438a2ed2d82629801a1",
      "en": "tos.text-unit.sid-9b96d50ad307440fb8f8629c0ba79bc1"
    },
    "z-convalescent": {
      "de": "tos.text-unit.sid-9b2a502ab9134c5996852ab4147fd4a5",
      "en": "tos.text-unit.sid-91a33c66d5014c579190a50f7a20c900"
    }
  }
};

// Route steps have their own questions and sources: the same graph node can
// occur in steps that ask about different works.  These independently read
// German and English entries are navigation aids, not a claim of alignment.
const passage = (ref, de, en) => ({kind: 'passage', ref, units: {de, en}});
const external = (ref, fragment) => ({kind: 'external', ref, ...(fragment ? {fragment} : {})});
const ROUTE_PASSAGES = {
  happyFate: passage('z-happy-isles', 'tos.text-unit.sid-f0bbac4efb584ebc83f3958179371e37', 'tos.text-unit.sid-41b2297091004d6f910c6b0b2cff2bfd'), // DE 951, EN 1111
  redemptionLimit: passage('z-redemption', 'tos.text-unit.sid-13ec69845f894a9a8738c6aad8b9945a', 'tos.text-unit.sid-83441b167d6e41e2b6d0ce47e622070d'), // DE 1663, EN 1845
  redemptionCreation: passage('z-redemption', 'tos.text-unit.sid-5289943ba03f4ffc99b5d4883a013d37', 'tos.text-unit.sid-711eb78d5a084dd29d6414a5901038bb'), // DE 1682, EN 1864
  gateway: passage('z-vision', 'tos.text-unit.sid-9a7e0d9b40004d93935f6141982e678e', 'tos.text-unit.sid-327b4400bd0e447ba87e7c9159ba06f3'), // DE 1848, EN 2037
  dwarf: passage('z-vision', 'tos.text-unit.sid-fd2e6ec6850c4ae0b0a72809e5cfc29f', 'tos.text-unit.sid-a9405bb910e24c8fbe2257a14f429bb4'), // DE 1852, EN 2041
  moment: passage('z-vision', 'tos.text-unit.sid-0bbb683276d04438a2ed2d82629801a1', 'tos.text-unit.sid-9b96d50ad307440fb8f8629c0ba79bc1'), // DE 1854, EN 2043
  imaginedCondition: passage('z-convalescent', 'tos.text-unit.sid-08dd3db818d7499ab2f49ddc63918689', 'tos.text-unit.sid-adff6765e69347bb91643ad56f18214a'), // DE 2649, EN 2848
  child: passage('z-metamorphoses', 'tos.text-unit.sid-d0f0508a4457499ea09763fe6b5896a4', 'tos.text-unit.sid-c45e515abb7d46bdb7938aaf51e022b2'), // DE 220, EN 355
  joy: passage('z-drunken-song', 'tos.text-unit.sid-ee40e720ddbb489a8780095c3e73ba7e', 'tos.text-unit.sid-45fc9a56887e414b84cd6bea443fa5ac'), // DE 3650, EN 3893
  woe: passage('z-drunken-song', 'tos.text-unit.sid-41d4f19238c94aab9e45f4f82d5f7780', 'tos.text-unit.sid-d33d1b97bb8946f7a461bdeb248570b2'), // DE 3642, EN 3885
  help: passage('z-pitiful', 'tos.text-unit.sid-444d81916a5346188a8950e178ef6621', 'tos.text-unit.sid-edac7900bae24b14a6630df30e88f209'), // DE 977, EN 1138
  obligation: passage('z-pitiful', 'tos.text-unit.sid-6056ccb5ce9d4d66badd94d136215dbf', 'tos.text-unit.sid-1528f69faeed417cae3e6e03f31cab88'), // DE 979, EN 1140
  qualifiedSong: passage('z-convalescent', 'tos.text-unit.sid-1220519740844f1f819b813d977fac54', 'tos.text-unit.sid-77e17877c73f4836888ccdae41468ef5'), // DE 2610, EN 2809
  apple: passage('z-convalescent', 'tos.text-unit.sid-313c312a17cd496891913e04b07f0f52', 'tos.text-unit.sid-2830327623174b5ebd7a4fd70286c141'), // DE 2598, EN 2797
  danceOfSpeech: passage('z-convalescent', 'tos.text-unit.sid-c47e5f66de4f4aa0a4b79cffb2036f0a', 'tos.text-unit.sid-8f8e8e3bd7aa4afd9aefbd3c243fc123'), // DE 2603, EN 2802
  replyToSong: passage('z-convalescent', 'tos.text-unit.sid-582dceac41464f55bf487b87d53c81e0', 'tos.text-unit.sid-137ef525a7b0420e920df2929e8965e2'), // DE 2614, EN 2813
  interrupted: passage('z-convalescent', 'tos.text-unit.sid-3fdfec5bbfae4b6b81922f4e04751742', 'tos.text-unit.sid-6da7e7cc992448e897d8f591a3d4b28d'), // DE 2635, EN 2834
  imaginedSpeech: passage('z-convalescent', 'tos.text-unit.sid-4b1380d94d65487eb4d189b64cfddf2a', 'tos.text-unit.sid-34398ee7749348ff98dd3483713c3a5d'), // DE 2643, EN 2842
  teacher: passage('z-convalescent', 'tos.text-unit.sid-e936534cf75f46caa321ebc5a0da1f66', 'tos.text-unit.sid-fd596cf581824d05a41526d2fb85e58c'), // DE 2641, EN 2840
  signDeparture: passage('z-sign', 'tos.text-unit.sid-1e5afcf46ca04fe7a4a20135223365bf', 'tos.text-unit.sid-91e301b459ee47bca18ce2f66767c9ad'), // DE 3687, EN 3931
  shepherd: passage('z-vision', 'tos.text-unit.sid-3bfa9f07f9614a4aa567c250edb713cd', 'tos.text-unit.sid-87fbf783426f4567a6a7f21c07326e45'), // DE 1868, EN 2057
  bite: passage('z-vision', 'tos.text-unit.sid-ae92a7183e344bb0998780e8dca0be10', 'tos.text-unit.sid-717a4cfe1b8e4c1ba3b01a44983b1caa'), // DE 1870, EN 2059
  laughter: passage('z-vision', 'tos.text-unit.sid-b1d488d8c2e24f32808e4e8912ed1102', 'tos.text-unit.sid-5902be75eed04d8f95862c2a862a88ab'), // DE 1877, EN 2066
  firstSeal: passage('z-seven-seals', 'tos.text-unit.sid-8edd0e4d04e94fbbaf17a0cffe8a08a5', 'tos.text-unit.sid-e64d9dbca7a0445ba87b72e0511523fa'), // DE 2740, EN 2965
  dance: passage('z-seven-seals', 'tos.text-unit.sid-3c72a017ec7f4c589d63aa4c7cfc5e3f', 'tos.text-unit.sid-276c6c80c0074727a341a082f7eacd5f'), // DE 2780, EN 3005
  refrain: passage('z-seven-seals', 'tos.text-unit.sid-f6cba59ce2844156a85aca65898a3e18', 'tos.text-unit.sid-bc6b5237009941b88eeda06e2061a992'), // DE 2784, EN 3009
  dogMemory: passage('z-vision', 'tos.text-unit.sid-bbb23084527c45a99e326e7fd2915506', 'tos.text-unit.sid-819184eac81b4e22900b57fced2de04b'), // DE 1862, EN 2051
  dogQuestion: passage('z-vision', 'tos.text-unit.sid-4bd873d518394ddd88421cf67e632091', 'tos.text-unit.sid-6a0116e5cd3447ab9a234b82ec87e851'), // DE 1865, EN 2054
  dogShepherd: passage('z-vision', 'tos.text-unit.sid-05b76ddd13b04db38f039da3a25b23d4', 'tos.text-unit.sid-a0b7dff3f51d457c8be880181feaa436'), // DE 1867, EN 2056
  companions: passage('z-convalescent', 'tos.text-unit.sid-6e1f4ec1f777414089432b62fa692e94', 'tos.text-unit.sid-e6a14205a3064198846e29ac7a1d7dd6'), // DE 2653, EN 2852
};

// Arrays correspond to the authored step order, with no inferred card fallback.
// External links are references only; in particular Camus points to an edition
// catalogue and does not promise an available full text.
export const ZARATHUSTRA_ROUTE_GROUNDS = {
  'fate-given-created': [ROUTE_PASSAGES.happyFate, external('camus-sisyphus'), external('camus-sisyphus'), external('gs341'), external('gs341'), ROUTE_PASSAGES.redemptionLimit, ROUTE_PASSAGES.redemptionLimit, ROUTE_PASSAGES.redemptionCreation],
  'moment-time': [ROUTE_PASSAGES.gateway, ROUTE_PASSAGES.gateway, ROUTE_PASSAGES.dwarf, ROUTE_PASSAGES.moment, external('spinoza1d7'), ROUTE_PASSAGES.happyFate],
  'repetition-difference': [ROUTE_PASSAGES.imaginedCondition, external('gs341'), external('deleuze-nietzsche-return'), external('deleuze-nietzsche-return', '#page=12'), external('deleuze-repetition'), ROUTE_PASSAGES.child],
  'affirmation-pain-action': [external('gs276'), ROUTE_PASSAGES.joy, ROUTE_PASSAGES.woe, ROUTE_PASSAGES.help, ROUTE_PASSAGES.obligation, ROUTE_PASSAGES.redemptionLimit, external('gs341')],
  'reading-voices': [ROUTE_PASSAGES.qualifiedSong, ROUTE_PASSAGES.apple, ROUTE_PASSAGES.danceOfSpeech, ROUTE_PASSAGES.replyToSong, ROUTE_PASSAGES.interrupted, ROUTE_PASSAGES.imaginedSpeech, ROUTE_PASSAGES.teacher, ROUTE_PASSAGES.signDeparture],
  'image-experience': [ROUTE_PASSAGES.shepherd, ROUTE_PASSAGES.bite, ROUTE_PASSAGES.laughter, ROUTE_PASSAGES.laughter, ROUTE_PASSAGES.firstSeal, ROUTE_PASSAGES.dance, ROUTE_PASSAGES.refrain, ROUTE_PASSAGES.dance],
  'memory-others': [ROUTE_PASSAGES.dogMemory, ROUTE_PASSAGES.dogMemory, ROUTE_PASSAGES.dogQuestion, ROUTE_PASSAGES.dogShepherd, ROUTE_PASSAGES.companions, ROUTE_PASSAGES.imaginedCondition, external('gs341')],
  'thinkers-necessity': [external('epictetus1'), ROUTE_PASSAGES.imaginedCondition, ROUTE_PASSAGES.happyFate, ROUTE_PASSAGES.happyFate, external('spinoza1d7'), external('spinoza3p6'), external('leibniz53-55'), external('leibniz53-55')],
};

/** Resolve a step's source, never a different selected graph card's source. */
export function createZarathustraRouteGroundTargets(document) {
  const versions = new Map((document?.versions ?? []).map(version => [version.id, version]));
  return (routeId, stepIndex, language) => {
    const choice = ZARATHUSTRA_ROUTE_GROUNDS[routeId]?.[stepIndex];
    if (!choice) return null;
    if (choice.kind !== 'passage') return {kind: choice.kind, ref: choice.ref, ...(choice.fragment ? {fragment: choice.fragment} : {})};
    const editionLanguage = String(language).toLowerCase().startsWith('en') ? 'en' : 'de';
    const version = GROUND_VERSIONS[editionLanguage];
    if (document?.id !== ZARATHUSTRA_DOCUMENT_ID || versions.get(version.id)?.revision !== version.revision) {
      return {kind: 'bibliographic', ref: choice.ref};
    }
    return {kind: 'exact', address: {documentId: document.id, versionId: version.id, revision: version.revision, unitId: choice.units[editionLanguage]}};
  };
}

export function createZarathustraGroundTargets(document) {
  const versions = new Map((document?.versions ?? []).map(version => [version.id, version]));
  return (ref, nodeId) => {
    if (document?.id !== ZARATHUSTRA_DOCUMENT_ID || !Object.hasOwn(CHAPTER_GROUNDS,ref)) return [];
    const chapter = CHAPTER_GROUNDS[ref];
    return Object.entries(GROUND_VERSIONS).flatMap(([language, expected]) => {
      if (versions.get(expected.id)?.revision !== expected.revision) return [];
      const paragraph = NODE_GROUND_PASSAGES[nodeId]?.[ref]?.[language];
      const unitId = paragraph ?? chapter.units[language];
      if (!unitId) return [];
      return [{language, title: chapter.title, scope: paragraph ? 'passage' : 'chapter',
        address: {documentId: document.id, versionId: expected.id, revision: expected.revision, unitId}}];
    });
  };
}
