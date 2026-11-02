/** Source-grounded reading connections for the atlas.
 * Grounds identify passages to read; they do not turn a connection into proof.
 * Own words are kept only for what a quotation cannot give: what happens at a place and who
 * speaks, how it joins the next place, the question it raises and the answer the book allows.
 * What a work itself says is never retold: a form cites the place instead ({ru, en, cite}),
 * and the stand shows the work's own text there. The map keeps the links the book itself makes
 * at a named place; everything else waits in bonsai/zarathustra/held.json to be grown together. */
export const RELATION_INQUIRY = {
  "atlas:moment:supports:gateway": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
        ]
      }
    ]
  },
  "atlas:moment:supports:two-paths": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
        ]
      }
    ]
  },
  "atlas:chapter-p3.r2:supports:dwarf": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-27decc16f1823c3d4dd1cde18d43a8c2","tos.text-unit.sid-70e02f3b222af3ebae475f634fd8f539"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-27decc16f1823c3d4dd1cde18d43a8c2","tos.text-unit.sid-70e02f3b222af3ebae475f634fd8f539"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
        ]
      }
    ]
  },
  "atlas:chapter-p3.r2:supports:shepherd": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-731a9e34e7c14198ab956f27436becc4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-731a9e34e7c14198ab956f27436becc4"]}
        ]
      }
    ]
  },
  "atlas:shepherd:develops:bite": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
        ]
      }
    ]
  },
  "atlas:bite:develops:laughter": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
        ]
      }
    ]
  },
  "atlas:dwarf:supports:circle": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
        ]
      }
    ]
  },
  "atlas:gateway:supports:two-paths": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
        ]
      }
    ]
  },
  "atlas:dwarf:supports:gateway": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-317301cd82d35cd0e2d4362b72783488","tos.text-unit.sid-38b6718fc95c36a467127712c215fff0"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-317301cd82d35cd0e2d4362b72783488","tos.text-unit.sid-38b6718fc95c36a467127712c215fff0"]}
        ]
      }
    ]
  },
  "atlas:gateway:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ]
  },
  "atlas:two-paths:supports:circle": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
        ]
      }
    ]
  },
  "atlas:shepherd:develops:laughter": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
        ]
      }
    ]
  },
  "atlas:present:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ]
  },
  "atlas:two-paths:questions:paths-opposition": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
        ]
      }
    ]
  },
  "atlas:gateway:questions:moment-returns": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ]
  },
  "atlas:shepherd:questions:shepherd-riddle": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-9ac5e60b3a9f3e21946551e7043e6f31","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-9ac5e60b3a9f3e21946551e7043e6f31","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
        ]
      }
    ]
  },
  "atlas:chapter-p3.r13:supports:animals": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
        ]
      }
    ]
  },
  "atlas:chapter-p3.r13:supports:nausea": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-0f8534033d9eb03f74ac4ffeb7d2bf6f","tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-0f8534033d9eb03f74ac4ffeb7d2bf6f","tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
        ]
      }
    ]
  },
  "atlas:all-things:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
        ]
      }
    ]
  },
  "atlas:same-life:supports:identity": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
        ]
      }
    ]
  },
  "atlas:all-things:supports:animals": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
        ]
      }
    ]
  },
  "atlas:wheel:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
        ]
      }
    ]
  },
  "atlas:nausea:supports:small-human": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ]
  },
  "atlas:recurrence:supports:becoming": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ]
  },
  "atlas:nausea:supports:body-resistance": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-0f8534033d9eb03f74ac4ffeb7d2bf6f","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-0f8534033d9eb03f74ac4ffeb7d2bf6f","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
        ]
      }
    ]
  },
  "atlas:animals:supports:wheel": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
        ]
      }
    ]
  },
  "atlas:animals:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ]
  },
  "atlas:animals:supports:ring": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
        ]
      }
    ]
  },
  "atlas:ring:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
        ]
      }
    ]
  },
  "atlas:nausea:supports:recurrence": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ]
  },
  "atlas:recurrence:supports:identity": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
        ]
      }
    ]
  },
  "atlas:recurrence:supports:fate": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ]
  },
  "atlas:fate:supports:task": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ]
  },
  "atlas:nausea:questions:nausea-return": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ]
  },
  "atlas:same-life:questions:same-life-return": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
        ]
      }
    ]
  },
  "atlas:recurrence:questions:circle-change": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
        ]
      }
    ]
  },
  "atlas:work:supports:local-return": {
    "warrant": {
      "ru": "Подобно солнцу, отдающему свой избыток, Заратустра хочет раздать мудрость и для этого нисходит к людям.",
      "en": "Like the sun giving its abundance, Zarathustra wants to share his wisdom and descends to people for that purpose.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-9b5926e8bcd6a467c72aaa8b8d51bcfb","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-prologue",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-9b5926e8bcd6a467c72aaa8b8d51bcfb","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"]}
        ]
      }
    ]
  },
  "atlas:affirmation:supports:joy-pain": {
    "warrant": {
      "ru": "Заратустра связывает «да» одной радости со всем горем: все вещи сцеплены и переплетены.",
      "en": "Zarathustra joins a yes to one joy to all sorrow: all things are chained and entwined.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
        ]
      }
    ]
  },
  "atlas:creation:develops:fate": {
    "warrant": {
      "ru": "Вспомнив свои прощания, Заратустра уточняет: его творящая воля хочет именно такой судьбы.",
      "en": "Recalling his farewells, Zarathustra makes his meaning more precise: his creative will wants precisely such a fate.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-382b87ebc0855954b7fee4c0e54e61c9","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-382b87ebc0855954b7fee4c0e54e61c9","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      }
    ]
  },
  "atlas:creation:supports:becoming": {
    "warrant": {
      "ru": "Заратустра славит становление и преходящее; творящий проходит через боль и превращение и оправдывает преходящее.",
      "en": "Zarathustra praises becoming and transience; the creator undergoes pain and transformation and justifies transience.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-8633a4e29cd08c654ad688a5aef20edc"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-8633a4e29cd08c654ad688a5aef20edc"]}
        ]
      }
    ]
  },
  "atlas:affirmation:supports:recurrence": {
    "warrant": {
      "ru": "Желание пережить мгновение дважды становится в песне желанием возвращения всех связанных вещей.",
      "en": "In the song, the wish to experience a moment twice becomes a wish for all linked things to return.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
        ]
      }
    ]
  },
  "atlas:joy-pain:supports:recurrence": {
    "warrant": {
      "ru": "Радость желает вечности; в песне к горю обращаются: «Пройди, но вернись!»",
      "en": "Joy wants eternity; in the song sorrow is addressed: “Pass away, but come back!”",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
        ]
      }
    ]
  },
  "atlas:dance:supports:ring": {
    "warrant": {
      "ru": "В шестой части песни добродетель танцора и превращение тяжёлого в лёгкое ведут к припеву о кольце возвращения.",
      "en": "In the song’s sixth movement, the dancer’s virtue and the becoming light of what is heavy lead to the refrain about the ring of recurrence.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-2178022befdc02bfa6f0e191d4eda013"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-2178022befdc02bfa6f0e191d4eda013"]}
        ]
      }
    ]
  },
  "atlas:joy-pain:questions:whole-part": {
    "warrant": {
      "ru": "Заратустра связывает желание повторить одну радость с возвращением всех переплетённых вещей.",
      "en": "Zarathustra connects the wish to repeat one joy with the return of all entwined things.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
        ]
      }
    ]
  },
  "atlas:creation:questions:will-fate": {
    "warrant": {
      "ru": "Заратустра связывает прощания своего пути с судьбой, которой хочет его творящая воля.",
      "en": "Zarathustra connects the farewells along his path with the fate his creative will wills.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-382b87ebc0855954b7fee4c0e54e61c9","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-382b87ebc0855954b7fee4c0e54e61c9","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
        ]
      }
    ]
  }
};
