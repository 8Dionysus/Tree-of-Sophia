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
