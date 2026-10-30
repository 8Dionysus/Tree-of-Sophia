/** Source-grounded reading connections for the atlas.
 * Grounds identify passages to read; they do not turn a connection into proof.
 * Own words are kept only for what a quotation cannot give (a reading, a
 * reconstructed argument, a distinction, a connection, a question, a method,
 * context). What a work itself says is never retold: a form cites the place
 * instead ({ru, en, cite}), and the stand shows the work's own text there —
 * the original and, through the alignment of the witnesses, the reader's
 * language (bonsai/zarathustra/witnesses.json, fragments.json). */
export const RELATION_INQUIRY = {
  "atlas:dossier:interprets:work": {
    "warrant": {
      "ru": "Подборка соединяет эти разные положения мысли внутри книги и позволяет читать их взаимное напряжение.",
      "en": "Bringing these passages together exposes the thought’s different positions within the book and lets their mutual tension be read.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
        ]
      }
    ],
    "question": {
      "ru": "Что второе место заставляет услышать в первом, чего не давала отдельная цитата?",
      "en": "What does the second passage make audible in the first that an isolated quotation did not?"
    }
  },
  "atlas:moment:supports:gateway": {
    "warrant": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-38b6718fc95c36a467127712c215fff0"],"s":23,"e":72},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"s":138},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"e":137},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"s":23}
        ]
      }
    ]
  },
  "atlas:moment:supports:two-paths": {
    "warrant": {
      "ru": "Переход к возврату происходит в вопросе о продолжении путей.",
      "en": "Recurrence enters through a question about following the paths further.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"s":73},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"],"e":136}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"],"s":73}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется вопрос о мгновении, когда нужно не только увидеть дороги, но и мысленно пройти дальше?",
      "en": "How does the question of the moment change when the paths must be followed in thought rather than merely seen?"
    }
  },
  "atlas:chapter-p3.r2:supports:dwarf": {
    "warrant": {
      "ru": "Связь формулы карлика с духом тяжести позволяет спросить, что именно герой отвергает в лёгкости этого ответа.",
      "en": "The formula’s connection with the spirit of gravity makes the ease of that answer a question.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-996c15507635248b4af03be863bead0f","tos.text-unit.sid-a1be3754ab91f1b4aca5afe353880904"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"],"s":52}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-8ab879484eca5fdebd313c28c48e4ce2","tos.text-unit.sid-70e02f3b222af3ebae475f634fd8f539"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"],"s":52},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"],"e":72}
        ]
      }
    ],
    "question": {
      "ru": "Почему тот же собеседник и отягощает восхождение, и делает ответ о времени слишком лёгким?",
      "en": "Why does the same interlocutor both burden the ascent and make the answer about time too easy?"
    }
  },
  "atlas:chapter-p3.r2:supports:shepherd": {
    "warrant": {
      "ru": "Поэтому глава даёт не готовую расшифровку, а переход между мыслью и образом.",
      "en": "So the chapter presents a movement between thought and image rather than a completed decoding.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"],"s":106},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-9ac5e60b3a9f3e21946551e7043e6f31","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"],"s":106},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa"],"e":35},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-9ac5e60b3a9f3e21946551e7043e6f31","tos.text-unit.sid-47ecea77ae8a30862d06ed7c80ebdf1a"],"s":120}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется, когда вопрос о времени становится вопросом о том, кто этот пастух?",
      "en": "What changes when the question about time becomes the question of who this shepherd is?"
    }
  },
  "atlas:shepherd:develops:bite": {
    "warrant": {
      "ru": "Поэтому сцена показывает переход силы действия, а не отвлечённый выбор одной добродетели.",
      "en": "So the scene shows a transfer into action rather than a selection of one abstract virtue.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"],"e":73}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"],"e":73}
        ]
      }
    ],
    "question": {
      "ru": "Почему призыв способен сделать то, чего не смогла сделать протянутая рука?",
      "en": "Why can the exhortation accomplish what the outstretched hand could not?"
    }
  },
  "atlas:bite:develops:laughter": {
    "warrant": {
      "ru": "Произошедшее меняет не только фигуру пастуха, но и отношение Заратустры к собственной жизни и смерти.",
      "en": "The event changes both the shepherd’s figure and Zarathustra’s relation to his own life and death.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":74},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38407ce82a4703428377d06e4af5c240","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      }
    ],
    "question": {
      "ru": "Что в этом смехе делает его предметом неутолимого желания рассказчика?",
      "en": "What makes this laughter the object of the narrator’s insatiable longing?"
    }
  },
  "atlas:chapter-p3.r13:supports:animals": {
    "warrant": {
      "ru": "Двойную роль зверей — заботу о герое и объяснение ему его учения — нужно удержать, читая их уверенность.",
      "en": "The animals’ assurance must be read within their double role of caring for Zarathustra and explaining his teaching to him.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca"],"s":249,"e":402},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-68da7bd14a262aac8d5b88c8007a76b2","tos.text-unit.sid-0bae010a9beb9126247aa70b03d2769b"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-1fdb6b338b3ca379b3fc0d4b9a295d84","tos.text-unit.sid-f54222b64288ce8a40f1eb5a1906385a"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-68da7bd14a262aac8d5b88c8007a76b2","tos.text-unit.sid-0bae010a9beb9126247aa70b03d2769b"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ]
  },
  "atlas:chapter-p3.r13:supports:nausea": {
    "warrant": {
      "ru": "Болезнь вводит в мысль о целом то, что не снимается общим знанием формулы; герой должен встретить именно повторение презираемого им.",
      "en": "Illness introduces into the thought of the whole something the general formula does not settle: the recurrence of what Zarathustra despises.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему мысль о возвращении становится трудной именно на малом человеке?",
      "en": "Why does the thought of recurrence become difficult precisely at the small human?"
    }
  },
  "atlas:all-things:supports:recurrence": {
    "warrant": {
      "ru": "Слова о целом охватывают сам ход изменений: повтор мыслится через становление, а не через остановку происходящего.",
      "en": "The song’s language of the whole encompasses change itself: repetition is articulated through becoming rather than arrested events.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"],"s":62}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
        ]
      }
    ]
  },
  "atlas:same-life:supports:identity": {
    "warrant": {
      "ru": "Эти три исключения задают силу слова «та же»: необходимо выяснить, что возвращается вместе с говорящим, а не довольствоваться сходством судеб.",
      "en": "These exclusions give “the same” its force: what returns with the speaker must be specified beyond resembling destinies.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Зачем перед словами о той же жизни перечислены именно эти спутники и этот мир?",
      "en": "Why are these particular companions and this world named before the same life?"
    }
  },
  "atlas:same-life:develops:whole-part": {
    "warrant": {
      "ru": "В §341 этот объём раскрыт ещё точнее, поэтому целое нельзя заменить избранным итогом.",
      "en": "Section 341 specifies this scope further, so a selected outcome cannot replace the whole.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-ce045055585467cfc09217fef32bfa81","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]},
        {"w":"die-froehliche-wissenschaft","loc":"341"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-ce045055585467cfc09217fef32bfa81","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Что исчезает из требования, если желать вернуть только смысл жизни, но не её ход?",
      "en": "What disappears from the demand if only life’s meaning is wanted back, without its course?"
    }
  },
  "atlas:all-things:supports:animals": {
    "warrant": {
      "ru": "Возврат к говорящим помещает всеобщую формулу в разговор с выздоравливающим. Содержание и способ усвоения не совпадают сразу.",
      "en": "Restoring the speakers places the universal formula in a conversation with a convalescent. Content and its uptake do not immediately coincide.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4","tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4"],"s":88},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f7a601c53e1f7c0cbd3e6e2121a128a","tos.text-unit.sid-067ece24c7569da43e43ac0b13a003c4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
        ]
      }
    ],
    "question": {
      "ru": "Что звери уже умеют произнести, а что Заратустра ещё должен рассказать от себя?",
      "en": "What can the animals already articulate, and what must Zarathustra still recount for himself?"
    }
  },
  "atlas:narrated-voices:develops:character-author": {
    "warrant": {
      "ru": "Эта речь внутри речи делает авторскую атрибуцию отдельной задачей: сначала нужно понять, зачем книга даёт формулу именно так.",
      "en": "Speech inside speech makes authorial attribution a separate task: first ask why the book presents the formula in this way.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"],"e":169}
        ]
      }
    ],
    "question": {
      "ru": "Что меняет воображаемая речевая рамка по сравнению с прямым признанием Заратустры?",
      "en": "What changes when an imagined speech frame replaces Zarathustra’s direct avowal?"
    }
  },
  "atlas:source-neighbor:supports:voice-reading": {
    "warrant": {
      "ru": "Соседние строки показывают, кто начал говорить, кому отвечает следующая реплика и где речь переходит в цитирование. В «Выздоравливающем» это позволяет услышать не только формулу зверей, но и ответ о том, как она стала песней.",
      "en": "Neighbouring lines show who began speaking, whom the next speech answers and where quotation begins. In “The Convalescent” this reveals both the animals’ formula and the reply about its becoming a song."
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
        ]
      }
    ]
  },
  "atlas:local-return:contrasts:recurrence": {
    "warrant": {
      "ru": "Первое возвращение меняет отношение к прежнему миру, второе требует уточнить полный объём повторения.",
      "en": "The first return changes a relation to the familiar world; the second asks after repetition’s full scope.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb"],"s":208},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-778b42e35fce3f7979a96c19045ceb9c","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-prologue",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"],"s":208}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Одинаков ли предмет слова «возвращение», когда возвращается путь к людям и когда возвращается вся жизнь?",
      "en": "Is recurrence’s object the same when a path back to people returns and when an entire life does?"
    }
  },
  "atlas:work:supports:local-return": {
    "warrant": {
      "ru": "Будущая встреча пока существует как направленность желания.",
      "en": "The coming encounter remains a direction of desire.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-87ee03096d4da574543c6c6cdeab9b26","tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb"]},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-86b4f442b37cc5ac7c949915868d53dd","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-prologue",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-87ee03096d4da574543c6c6cdeab9b26","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему достигнутое в уединении требует выхода из уединения?",
      "en": "Why does what was attained in solitude require leaving solitude?"
    }
  },
  "atlas:dossier:develops:open-reading": {
    "warrant": {
      "ru": "Один мотив проходит через вопрос к карлику, видение пастуха и песню зверей. Сравнение даёт основания различить космологическую формулу, испытание говорящего и работу образа, не распределяя их заранее по отдельным книгам.",
      "en": "One motif passes through questions to the dwarf, the shepherd’s vision and the animals’ song. Comparison grounds a distinction among cosmological formulation, the speaker’s ordeal and the work of imagery within the same book.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое прочтение связывает смену этих форм, а не просто выбирает одну из них?",
      "en": "Which reading connects these changing forms rather than simply selecting one?"
    }
  },
  "atlas:narrated-voices:develops:source-neighbor": {
    "warrant": {
      "ru": "Распознать несколько голосов значит вернуться к словам, которыми один голос передаёт другой. Концовка «Выздоравливающего» даёт точный случай.",
      "en": "Recognising several voices means returning to the words by which one voice presents another. The closing of “The Convalescent” offers a precise case.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"],"s":170,"e":321}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ]
  },
  "atlas:gateway:develops:present": {
    "warrant": {
      "ru": "Ворота становятся этим мгновением. Настоящее здесь не пустая граница времён: вопрос о его возвращении включает конкретную сцену, в которой он поставлен.",
      "en": "The gateway becomes this moment. The present is not an empty temporal boundary: asking about its return includes the particular scene asking it.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"],"s":138}
        ]
      }
    ],
    "question": {
      "ru": "Что добавляют подробности к вопросу, который можно было бы задать о времени вообще?",
      "en": "What do the details add to a question that could have been asked about time in general?"
    }
  },
  "atlas:two-paths:supports:temporal-knot": {
    "warrant": {
      "ru": "«Узел» помогает проследить этот переход от пространственной встречи к мысли о связанности мгновения с будущим.",
      "en": "The knot helps trace the movement from spatial meeting to the moment’s connection with what follows.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"e":137},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"],"e":119}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"e":137},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"],"e":119}
        ]
      }
    ],
    "question": {
      "ru": "Где в речи встреча дорог превращается в утверждаемую связь вещей?",
      "en": "Where does the paths’ meeting become a proposed connection among things in the speech?"
    }
  },
  "atlas:dwarf:develops:circle": {
    "warrant": {
      "ru": "Читать круг стоит внутри этого продолжения.",
      "en": "The circle should be read within this continuation.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb"]}
        ]
      }
    ],
    "question": {
      "ru": "Что требует продолжения после того, как карлик уже назвал форму времени?",
      "en": "What still demands continuation after the dwarf has named time’s shape?"
    }
  },
  "atlas:circle:supports:cosmological": {
    "warrant": {
      "ru": "Назвать время кругом — придать возвращению всеобщую форму. Космологическое чтение должно пройти переходы между вопросами Заратустры.",
      "en": "Calling time a circle gives recurrence a universal form. A cosmological reading must follow the transitions between Zarathustra’s questions.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-aa80ebe6fb9de20b613cb4f53ed7e855"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ],
    "question": {
      "ru": "Какие основания добавляет дальнейшая речь к одной круговой форме?",
      "en": "What grounds does the subsequent speech add to the circular form alone?"
    }
  },
  "atlas:dwarf:supports:voice-reading": {
    "warrant": {
      "ru": "Смена интонации принадлежит самой постановке вопроса: формула звучит иначе у того, кто считает её лёгкой, и у того, кто страшится её смысла.",
      "en": "Tone belongs to the question’s staging: the formula differs between one who finds it easy and one afraid of its significance.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"],"e":51},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"],"e":72},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"],"e":105}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"]}
        ]
      }
    ]
  },
  "atlas:wheel:interprets:recurrence": {
    "warrant": {
      "ru": "Образ колеса удерживает движение и повтор вместе: существование не сохраняется неподвижным, а получает вращающийся ритм изменений.",
      "en": "The image of the wheel holds movement and recurrence together: existence is not preserved motionless but given a rotating rhythm of changes.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"],"e":61}
        ]
      }
    ],
    "question": {
      "ru": "Почему возвращение поётся через движения, а не через неизменность вещи?",
      "en": "Why is recurrence sung through movements rather than a thing’s immutability?"
    }
  },
  "atlas:animals:develops:cosmological": {
    "warrant": {
      "ru": "Именно этот переход даёт космологическому чтению определённый предмет: возвращение не одного переживания, а целого порядка.",
      "en": "It is this transition that gives cosmological reading a definite object: the recurrence of an entire order rather than one experience.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ],
    "question": {
      "ru": "Что объяснение великого года добавляет к песне о колесе?",
      "en": "What does the great year’s explanation add to the wheel’s song?"
    }
  },
  "atlas:nausea:develops:joy-pain": {
    "warrant": {
      "ru": "Между этими местами возникает вопрос, как меняется отношение к целому, которое прежде было невыносимо.",
      "en": "Between these passages lies the question of a changed relation to a previously unbearable whole.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      },
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-d2d3ae311641b1822df4faf4848eb74a","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
        ]
      }
    ],
    "question": {
      "ru": "Что в поздней песне позволяет произнести то, что в сцене болезни вызывало отвращение?",
      "en": "What in the later song allows saying what provoked disgust in the illness scene?"
    }
  },
  "atlas:bite:interprets:creation": {
    "warrant": {
      "ru": "Сопоставление уточняет, какую ступень перемены показывает действие в видении.",
      "en": "Comparison asks which stage of change the visionary act presents.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
        ]
      },
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
        ]
      }
    ],
    "question": {
      "ru": "Видим ли мы в укусе уже создание или условие того, что появляется вслед за ним?",
      "en": "Does the bite show creation itself or the condition of what appears after it?"
    }
  },
  "atlas:laughter:interprets:affirmation": {
    "warrant": {
      "ru": "«Семь печатей» дают конкретное соседство для чтения смеха как утверждения.",
      "en": "“The Seven Seals” gives a specific textual neighbour for reading laughter as affirmation.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-33bdb4e7a58e8447fcff393b590cd365","tos.text-unit.sid-33bdb4e7a58e8447fcff393b590cd365"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-187d71e13bc9881187035bebd261de0c","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-33bdb4e7a58e8447fcff393b590cd365","tos.text-unit.sid-33bdb4e7a58e8447fcff393b590cd365"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
        ]
      }
    ],
    "question": {
      "ru": "Как желание этого смеха соотносится с позднейшим пением собственного «да»?",
      "en": "How does longing for this laughter relate to later singing one’s own yes?"
    }
  },
  "atlas:ring:interprets:whole-part": {
    "warrant": {
      "ru": "Кольцо получает смысл через это расширение желаемого.",
      "en": "The ring gains meaning through that expansion of what is wanted.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-aa5257a8fe54500bcc86849f77bfe5c2","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      },
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
        ]
      }
    ],
    "question": {
      "ru": "Как отдельное желание становится в песне желанием целого?",
      "en": "How does a particular desire become desire for the whole within the song?"
    }
  },
  "atlas:dance:supports:performative": {
    "warrant": {
      "ru": "Переход к исполнению следует движению самой песни: отношение к вечности должно обрести голос и движение.",
      "en": "Enactment follows the song’s own movement: the relation to eternity seeks voice and motion.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-40d09c0747d9bac0cf78568ff49e8cd0","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"],"s":120}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-40d09c0747d9bac0cf78568ff49e8cd0","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"],"s":120}
        ]
      }
    ],
    "question": {
      "ru": "Что здесь песня делает со словом утверждения, кроме сообщения его содержания?",
      "en": "What does the song do with affirmation beyond communicating its content?"
    }
  },
  "atlas:gateway:supports:nonlinear": {
    "warrant": {
      "ru": "Нелинейное чтение следует этому устройству взгляда: нынешний вопрос оказывается и внутри времени, и направленным на его целое.",
      "en": "A nonlinear reading follows this arrangement: the present question is both within time and directed toward its whole.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-7b662f8453b880f152323256b23c2409"],"s":23},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-38b6718fc95c36a467127712c215fff0"],"s":23,"e":72},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется положение читателя, когда спрашивающий тоже включён в то, о чём спрашивает?",
      "en": "How does the reader’s position change when the questioner is included in what he questions?"
    }
  },
  "atlas:wheel:compares:circle": {
    "warrant": {
      "ru": "Сравнение различает краткий ответ о форме и развёрнутое изображение процесса, не сводя обоих говорящих к одному тезису.",
      "en": "Comparison distinguishes a brief answer about shape from an unfolding process without reducing both speakers to one thesis.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"],"s":52},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"],"e":61}
        ]
      }
    ],
    "question": {
      "ru": "Что прибавляется к кругу, когда возвращение начинает двигаться в песне?",
      "en": "What is added to the circle when recurrence begins to move in a song?"
    }
  },
  "atlas:ring:compares:circle": {
    "warrant": {
      "ru": "Похожая замкнутая форма выполняет разную работу: объясняет устройство и связывает желание.",
      "en": "A similar closed form does different work: explaining an arrangement and binding a desire.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"]}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему замена брачного кольца геометрическим кругом обеднит именно эту песнь?",
      "en": "Why would replacing the marriage ring with a geometrical circle impoverish this particular song?"
    }
  },
  "atlas:ring:interprets:affirmation": {
    "warrant": {
      "ru": "Кольцо связывает эти названные возможности говорящего в повторяемое утверждение: желание звучит вновь, меняя своё образное окружение.",
      "en": "The ring binds the speaker’s named possibilities into repeated affirmation: desire sounds again within changing imagery.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-aa5257a8fe54500bcc86849f77bfe5c2","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-aa5257a8fe54500bcc86849f77bfe5c2","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      }
    ],
    "question": {
      "ru": "Что делает повтор припева с разными основаниями, из которых он каждый раз возникает?",
      "en": "What does the refrain’s repetition do to the different grounds from which it arises?"
    }
  },
  "atlas:nausea:supports:small-human": {
    "warrant": {
      "ru": "Отвращение направлено не на абстрактный круг. Так глава называет трудность, которую общая формула зверей способна скрыть своей лёгкостью.",
      "en": "Disgust concerns more than an abstract cycle. The chapter names a difficulty that the animals’ general formula can conceal through its ease.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-e5c2cd07214c95f19a1a4dcf30a074b1","tos.text-unit.sid-e5c2cd07214c95f19a1a4dcf30a074b1"],"s":42},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ]
  },
  "atlas:dance:supports:form-content": {
    "warrant": {
      "ru": "Форма участвует в переходе: читателю предлагают не только понять высказывание, но услышать изменение его способа звучать.",
      "en": "Form participates in the transition: the reader is invited to hear a change in utterance as well as understand a statement.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-40d09c0747d9bac0cf78568ff49e8cd0","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"],"s":120}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-ab141acd46dabfd26bd179718bed791b","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      }
    ],
    "question": {
      "ru": "Что не сохранится, если эти два раздела заменить определением утверждения?",
      "en": "What would be lost by replacing these two sections with a definition of affirmation?"
    }
  },
  "atlas:cosmological:interprets:recurrence": {
    "warrant": {
      "ru": "Космологическое чтение удерживает именно этот объём и прослеживает, как вопросы о связанности вещей у ворот переходят в описание повторяющегося порядка.",
      "en": "Cosmological reading retains that scope and follows how the gateway’s questions about connected things become a description of a recurring order.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-aa80ebe6fb9de20b613cb4f53ed7e855"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      },
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"],"e":119}
        ]
      }
    ]
  },
  "atlas:existential:interprets:same-life": {
    "warrant": {
      "ru": "Сопоставление с формулой зверей позволяет читать тождество жизни как содержание личного испытания, а не только описание последовательности.",
      "en": "Beside the animals’ formula, life’s identity becomes the content of a personal trial as well as a description of sequence.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему точность повторяемого здесь важна для ответа живущего?",
      "en": "Why does the exactness of what recurs matter to the living person’s answer?"
    }
  },
  "atlas:poetic:interprets:ring": {
    "warrant": {
      "ru": "Поэтическое чтение исследует, как возвращение становится образом союза и плодотворности, а не только повторяющейся формы.",
      "en": "Poetic reading asks how recurrence becomes an image of union and fruitfulness beyond a repeating shape.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
        ]
      }
    ],
    "question": {
      "ru": "Как образ плодотворности меняет ожидание, будто возвращение означает только повтор готового?",
      "en": "How does fruitfulness change the expectation that recurrence merely repeats what is already formed?"
    }
  },
  "atlas:temporal-knot:develops:necessity": {
    "warrant": {
      "ru": "Сопоставление различает необходимость, выводимую в рассуждении, и бессилие перед уже бывшим, переживаемое волей.",
      "en": "Comparison distinguishes an argued necessity from the will’s experienced inability before what has been.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-aa80ebe6fb9de20b613cb4f53ed7e855"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      },
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]}
        ]
      }
    ],
    "question": {
      "ru": "Одинаковую ли невозможность выражают «это должно вернуться» и «я не могу хотеть назад»?",
      "en": "Do “this must return” and “I cannot will backwards” express the same kind of impossibility?"
    }
  },
  "atlas:voice-reading:interprets:animals": {
    "warrant": {
      "ru": "Чтение по голосам следует росту их уверенности и сопоставляет её с молчанием героя, не заменяя это соотношение заранее выбранным согласием или отрицанием.",
      "en": "Reading by voices follows their growing assurance beside his silence without replacing that relation with predetermined agreement or rejection.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
        ]
      }
    ],
    "question": {
      "ru": "Что звери берутся знать о Заратустре сверх содержания его учения?",
      "en": "What do the animals claim to know about Zarathustra beyond his teaching’s content?"
    }
  },
  "atlas:performative:develops:affirmation": {
    "warrant": {
      "ru": "Эти места позволяют исследовать переход от услышанного содержания к способу отвечать ему всем существованием.",
      "en": "Together they open the passage from heard content to a way of answering with one’s existence.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-aa5257a8fe54500bcc86849f77bfe5c2","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-aa5257a8fe54500bcc86849f77bfe5c2","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое различие между знанием формулы и её усвоением показывают вопрос и песнь?",
      "en": "What difference between knowing the formula and taking it up do the question and song show?"
    }
  },
  "atlas:selective:questions:whole-part": {
    "warrant": {
      "ru": "Выбор отдельных частей меняет предмет испытания: вопрос обращён к этой последовательности целиком, а не к составленной заново желательной жизни.",
      "en": "Selecting parts changes the trial’s object: it concerns this entire sequence rather than a desirable life assembled anew.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}],
    "question": {
      "ru": "Что вы изменили бы в самом вопросе демона, исключив один возвращающийся эпизод?",
      "en": "What would excluding one recurring episode change in the demon’s question itself?"
    }
  },
  "atlas:small-human:develops:human-overcoming": {
    "warrant": {
      "ru": "Преодоление оказывается поставлено внутри той же жизни, где возвращается малое; напряжение нельзя снять простым обещанием, что нежеланное однажды исчезнет.",
      "en": "Overcoming is posed within the same life in which smallness returns; a promise that the unwanted will disappear does not resolve the tension.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
        ]
      }
    ],
    "question": {
      "ru": "Как понимать преодоление, если сама речь о нём должна вновь прозвучать в той же жизни?",
      "en": "How is overcoming to be understood if its proclamation must sound again in the same life?"
    }
  },
  "atlas:nonlinear:interprets:present": {
    "warrant": {
      "ru": "Настоящее читается через эти обращения, где движение рассказа не совпадает с одной прямой последовательностью событий.",
      "en": "The present is read through these returns, where narrative movement exceeds one straight event sequence.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-7288d1aabe7691c1bc8d3619c3e8ef24"]}
        ]
      }
    ],
    "question": {
      "ru": "Что связывает нынешний вой с детским воспоминанием — совпадение события или возвращение переживания?",
      "en": "What connects the present howl with childhood: a matching event or a returning experience?"
    }
  },
  "atlas:form-content:develops:proposition-poem": {
    "warrant": {
      "ru": "Сопоставление позволяет проверить, какую работу тезис, вопрос и образ совершают по-разному.",
      "en": "Comparison examines the different work done by proposition, question and image.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"],"s":52},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-40d09c0747d9bac0cf78568ff49e8cd0","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"],"s":120}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-40d09c0747d9bac0cf78568ff49e8cd0","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"]}
        ]
      }
    ],
    "question": {
      "ru": "Какой переход исчезает при пересказе всех трёх форм одним утверждением?",
      "en": "Which transition disappears when all three forms are paraphrased as one assertion?"
    }
  },
  "atlas:task:develops:teach-live": {
    "warrant": {
      "ru": "Передать мысль здесь означает вступить в задачу, которая затрагивает самого передающего, а не только сообщить слушателю формулу.",
      "en": "Conveying the thought means entering a task that affects its bearer, beyond communicating a formula to a listener.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему способность сказать учение ещё не завершает судьбу того, кто должен его возвестить?",
      "en": "Why does being able to state the teaching not complete the fate of its herald?"
    }
  },
  "atlas:open-reading:develops:cosmos-evidence": {
    "warrant": {
      "ru": "Вопросы о связанности мира у ворот и вопрос демона о действии имеют разный адрес. Сравнение прочтений начинается с этой разницы: что именно текст просит мыслить как устройство мира и что предлагает испытать на собственном ответе.",
      "en": "The gateway’s questions about the world’s connections and the demon’s question about action address different objects. Comparing readings begins here: what does the text ask us to think as the world’s arrangement, and what to test in our own response?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"die-froehliche-wissenschaft","loc":"341"}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "На какой из этих вопросов отвечает выбранный вами довод?",
      "en": "Which of these questions does your chosen argument answer?"
    }
  },
  "atlas:recurrence:questions:becoming": {
    "warrant": {
      "ru": "Вопрос возникает не из противопоставления повтора неподвижности, а из того, как возвращается сама последовательность изменений.",
      "en": "The question concerns the recurrence of change’s sequence itself, not a contrast between repetition and immobility.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"],"s":62},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"],"s":62},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47"]}
        ]
      }
    ],
    "question": {
      "ru": "В чём различаются изменение внутри великого года и изменение самого возвращающегося года?",
      "en": "How does change within the great year differ from change in the year that returns?"
    }
  },
  "atlas:identity:compares:difference": {
    "warrant": {
      "ru": "Сравнение ставит вопрос к самому критерию тождества: какой состав должен совпасть, а какое изменение принадлежит способу быть тем же.",
      "en": "Comparison questions identity’s criterion: what must match, and what change belongs to a way of remaining the same?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]},
        {"w":"heraclitus-book","loc":"B12"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      },
      {"ref":"heraclitus-river","cite":[{"w":"heraclitus-book","loc":"B12"}]}
    ],
    "question": {
      "ru": "Одинаково ли устроено «то же» в этих двух высказываниях?",
      "en": "Is the same constituted in the same way in these two statements?"
    }
  },
  "atlas:fate:questions:agency": {
    "warrant": {
      "ru": "Эта перестановка позволяет спросить, как действующий принадлежит тому, что называет судьбой.",
      "en": "This reversal asks how an agent belongs to what he calls fate.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-8633a4e29cd08c654ad688a5aef20edc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется между «моя воля — моя судьба» и «моя воля хочет этой судьбы»?",
      "en": "What changes between “my will is my fate” and “my will wants this fate”?"
    }
  },
  "atlas:necessity:develops:free-necessary": {
    "warrant": {
      "ru": "Определение заставляет различить необходимость и принуждение прежде, чем считать их одним препятствием действованию.",
      "en": "The definition requires distinguishing necessity from constraint before treating them as one obstacle to activity.",
      "cite": [{"w":"ethica","loc":"I.D7"}]
    },
    "grounds": [{"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}],
    "question": {
      "ru": "Как определение меняет вопрос, если свободу первоначально понимали как отсутствие всякой необходимости?",
      "en": "How does the definition change the question if freedom initially meant absence of all necessity?"
    }
  },
  "atlas:creation:develops:same-new": {
    "warrant": {
      "ru": "Возвращающийся образ движения здесь соединён с различием между завоеванием возможности и началом самой игры.",
      "en": "Recurrent movement joins the distinction between winning a possibility and beginning the game itself.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]},
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему новое начинание изображено колесом, а не просто разрывом с прежним?",
      "en": "Why is a new beginning pictured as a wheel rather than merely a break with the earlier?"
    }
  },
  "atlas:memory:develops:again-memory": {
    "warrant": {
      "ru": "Текст показывает само рождение узнавания.",
      "en": "The text shows recognition arising.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-7288d1aabe7691c1bc8d3619c3e8ef24"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb","tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa"],"s":82}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
        ]
      }
    ],
    "question": {
      "ru": "Что именно узнаётся здесь первым — событие, звук или собственное чувство?",
      "en": "What is recognised first here: an event, a sound or one’s own feeling?"
    }
  },
  "atlas:amor-fati:develops:affirmation": {
    "warrant": {
      "ru": "Связь разворачивает программу учения и изменения взгляда; текст ещё не описывает законченный механизм этого перехода.",
      "en": "The connection unfolds a programme of learning and changed vision without a completed mechanism of transition.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"276"}]
    },
    "grounds": [{"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}],
    "question": {
      "ru": "Как в этом желании связаны видение прекрасного и делание прекрасным?",
      "en": "How are seeing beauty and making beautiful connected in this wish?"
    }
  },
  "atlas:becoming:develops:circle-change": {
    "warrant": {
      "ru": "Становление и повтор принадлежат разным уровням описания в речах зверей: меняются состояния, возвращается их ход.",
      "en": "Becoming and recurrence occupy different descriptive levels of the animals’ speeches: states change and their course returns.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"],"e":79},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"],"e":79},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое различие пришлось бы признать между оборотами, чтобы сама эта формула изменилась?",
      "en": "Which difference between revolutions would alter the formula itself?"
    }
  },
  "atlas:present:compares:memory": {
    "warrant": {
      "ru": "Настоящее соединяет узнавание с появлением нового образа; память не заполняет сцену заранее до конца.",
      "en": "The present joins recognition with a new image’s appearance; memory does not fill out the scene in advance.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-7288d1aabe7691c1bc8d3619c3e8ef24"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"],"e":52}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea"],"e":64},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"],"e":52}
        ]
      }
    ],
    "question": {
      "ru": "Как знакомый звук приводит к тому, чего рассказчик ещё никогда не видел?",
      "en": "How does a familiar sound lead to what the narrator has never seen before?"
    }
  },
  "atlas:affirmation:develops:joy-pain": {
    "warrant": {
      "ru": "Утверждение расширяется в самой песне: важно понять этот ход желания, а не заменить его заранее удобной оценкой отдельных эпизодов.",
      "en": "Affirmation expands within the song itself: its movement of desire must be understood rather than replaced by a convenient appraisal of separate episodes.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-2a8aada5d848098eed8b2b160fafa871","tos.text-unit.sid-2a8aada5d848098eed8b2b160fafa871"]},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-b72032847012285d32829661efad3ea3","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-a0d84a7b3f6d5093f616ea7159f64287","tos.text-unit.sid-02dd16795cd0068789a785a0f5e9787e"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-d2d3ae311641b1822df4faf4848eb74a","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
        ]
      }
    ],
    "question": {
      "ru": "Как песня переходит от «радость хочет себя» к «радость хочет всего»?",
      "en": "How does the song move from joy wanting itself to joy wanting everything?"
    }
  },
  "atlas:task:interprets:dossier": {
    "warrant": {
      "ru": "Подборка позволяет проследить, как названное другими предназначение соотносится с собственным временем действовать.",
      "en": "A dossier can follow how a vocation named by others relates to one’s own time to act.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]},
        {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"],"s":48}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
        ]
      },
      {
        "ref": "z-sign",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-550efec95e443c4bd2b7c2897f5a45c2","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
        ]
      }
    ],
    "question": {
      "ru": "Что должно произойти между называнием задачи и наступлением своего часа?",
      "en": "What must happen between naming a task and the arrival of one’s own hour?"
    }
  },
  "atlas:cosmological:compares:existential": {
    "warrant": {
      "ru": "Сравнение удерживает связь двух вопросов: что повторяется и каким существом нужно стать, чтобы желать этого повторения.",
      "en": "Comparison retains two connected questions: what recurs, and what one must become to want its recurrence.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
        {"w":"die-froehliche-wissenschaft","loc":"341"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Зачем личному испытанию нужна настолько строгая формула повторяемого?",
      "en": "Why does the personal trial need such a strict account of what repeats?"
    }
  },
  "atlas:poetic:compares:cosmological": {
    "warrant": {
      "ru": "Их сопоставление показывает, как речь об одном мотиве меняет задачу между объяснением и песенным желанием.",
      "en": "Comparison shows the motif’s task changing between explanation and sung desire.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47"]}
        ]
      }
    ],
    "question": {
      "ru": "Что повторяемое объяснение ещё не говорит о желании вечности?",
      "en": "What does explaining recurrence leave unsaid about desiring eternity?"
    }
  },
  "atlas:selective:questions:existential": {
    "warrant": {
      "ru": "Если испытание оставить только для приятных эпизодов, исчезнет переход от исключительного счастливого мгновения к повседневному действованию.",
      "en": "Restricting the trial to pleasant episodes removes the transition from an exceptional happy moment to everyday activity.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}],
    "question": {
      "ru": "Как исключительное мгновение должно изменить отношение ко всему остальному?",
      "en": "How would an exceptional moment have to change one’s relation to everything else?"
    }
  },
  "atlas:fatalism:questions:agency": {
    "warrant": {
      "ru": "Поэтому вопрос об участии поставлен внутри повторяемого порядка: действующий не изображён внешним зрителем уже готовой судьбы.",
      "en": "Participation is therefore posed within the recurring order: the agent is not depicted as an outside spectator of a ready-made fate.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"],"s":105}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется в слове «суждено», если действующий сам входит в причинную связь?",
      "en": "What changes in “destined” when the agent belongs within the causal nexus?"
    }
  },
  "atlas:affirmation:contrasts:fatalism": {
    "warrant": {
      "ru": "Эти места дают проверить вывод от необходимости к безучастию, но не разрешают заранее весь вопрос о действии.",
      "en": "These passages test the inference from necessity to non-participation without settling the whole question of action.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"276"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"],"s":105}
      ]
    },
    "grounds": [
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]},
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"],"s":105}
        ]
      }
    ],
    "question": {
      "ru": "Что именно должно стать напрасным, если действие тоже входит в необходимый ход?",
      "en": "What exactly would become futile if action itself belongs to the necessary course?"
    }
  },
  "atlas:same-new:questions:difference": {
    "warrant": {
      "ru": "Сопоставление переносит вопрос с наличия перемен на их роль: составляют ли различия только содержание цикла или участвуют в объяснении самого возврата?",
      "en": "Comparison shifts the question from whether change occurs to its role: are differences only a cycle’s contents, or do they help explain recurrence itself?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]},
        {"w":"nietzsche-et-la-philosophie","loc":"II.5"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]}
        ]
      },
      {
        "ref": "deleuze-nietzsche-return",
        "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
      }
    ]
  },
  "atlas:circle-change:questions:wheel": {
    "warrant": {
      "ru": "Круговое движение здесь вмещает изменения; следующий вопрос касается того, повторяется ли именно их последовательность.",
      "en": "Circular movement accommodates changes here; the next question concerns whether their sequence itself repeats.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ],
    "question": {
      "ru": "Как объяснение великого года уточняет то, что образ катящегося колеса оставляет открытым?",
      "en": "How does the account of the great year specify what the rolling wheel leaves open?"
    }
  },
  "atlas:free-necessary:questions:fate": {
    "warrant": {
      "ru": "Поправка меняет отношение между полученным и желаемым, сохраняя его трудность; её нужно прочитать вместе с утверждением о творящем освобождении воли.",
      "en": "The correction changes the relation between what is received and wanted while retaining its difficulty; it must be read together with his account of willing’s creative liberation.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему первой формулировки о воле как судьбе оказывается недостаточно?",
      "en": "Why is the first formulation of willing as fate insufficient?"
    }
  },
  "atlas:joy-pain:questions:existential": {
    "warrant": {
      "ru": "Жизненная проба возникает между этими двумя движениями: полным составом возвращаемого и расширением желания.",
      "en": "The existential trial arises between these movements: recurrence’s full contents and desire’s expansion.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860","tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860"]}
        ]
      }
    ],
    "question": {
      "ru": "Что позволяет радости одного мгновения распространить своё хотение на весь ход жизни?",
      "en": "What allows one moment’s joy to extend its willing over an entire life’s course?"
    }
  },
  "atlas:teach-live:questions:nausea": {
    "warrant": {
      "ru": "Название задачи не отменяет рассказанного испытания: глава связывает способность учить с тем, что пришлось пережить самому говорящему.",
      "en": "Naming the task does not erase that ordeal: the chapter connects teaching with what the speaker himself had to undergo.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
        ]
      }
    ],
    "question": {
      "ru": "Как собственное испытание должно войти в будущую речь учителя?",
      "en": "How must the teacher’s own ordeal enter his future speech?"
    }
  },
  "atlas:human-overcoming:questions:laughter": {
    "warrant": {
      "ru": "Преодоление входит в сцену через смену именования и неслыханный смех; заключительное желание Заратустры показывает, что эта перемена остаётся впереди него.",
      "en": "Overcoming enters through changed naming and unprecedented laughter; Zarathustra’s closing longing places that change still ahead of him.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      }
    ],
    "question": {
      "ru": "Что даёт смена имени, чего ещё не объясняет описание самого укуса?",
      "en": "What does the change of name contribute beyond the description of the bite?"
    }
  },
  "atlas:proposition-poem:questions:cosmological": {
    "warrant": {
      "ru": "Извлечение космологического тезиса должно сохранить последовательность реплик в главе.",
      "en": "Extracting a cosmological thesis must retain the sequence of speeches in the chapter.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется тезис, если читать его после ответа героя, а не отдельно от сцены?",
      "en": "How does the thesis change when read after the protagonist’s reply rather than apart from the scene?"
    }
  },
  "atlas:character-author:questions:task": {
    "warrant": {
      "ru": "Эти разные способы назвать призвание позволяют исследовать авторскую композицию через распределение речи, а не через немедленное отождествление голосов.",
      "en": "These ways of naming a vocation let us examine authorial composition through the allocation of speech, not through an immediate identification of the voices.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]},
        {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
        ]
      },
      {
        "ref": "z-sign",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
        ]
      }
    ],
    "question": {
      "ru": "Какую связь между двумя сценами можно обосновать, не стирая разницу говорящих?",
      "en": "What connection between the scenes can be established while preserving their different speakers?"
    }
  },
  "atlas:again-memory:questions:identity": {
    "warrant": {
      "ru": "Это два разных основания слова «снова»: устройство предполагаемого повторения и узнавание внутри переживаемой сцены.",
      "en": "These ground “again” differently: the proposed recurrence’s structure and recognition within an experienced scene.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
        ]
      }
    ],
    "question": {
      "ru": "Зачем для утверждения тождества нужно узнавание — и нужно ли оно в обоих случаях?",
      "en": "What would recognition contribute to identity, and is it needed in both cases?"
    }
  },
  "atlas:whole-part:questions:recurrence": {
    "warrant": {
      "ru": "Паутина мелких подробностей в §341 удерживает точность целого. Два места сближают полноту состава и полноту желания.",
      "en": "Section 341’s small particulars preserve the whole’s exactness. The passages join completeness of contents with completeness of desire.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
        ]
      }
    ]
  },
  "atlas:cosmos-evidence:questions:necessity": {
    "warrant": {
      "ru": "Обоснование возврата начинается в этих переходах между возможностью, вечностью и связанностью; их вопросительная форма сохраняет работу для читателя.",
      "en": "Recurrence’s warrant begins in these moves between possibility, eternity and connectedness; their interrogative form leaves work for the reader.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ],
    "question": {
      "ru": "Какой переход в этой цепочке требует самостоятельного основания?",
      "en": "Which transition in this chain requires an independent ground?"
    }
  },
  "atlas:heraclitus:compares:becoming": {
    "warrant": {
      "ru": "Сравнение позволяет различить сохраняемое имя целого и меняющийся состав, прежде чем спрашивать о повторе всей последовательности.",
      "en": "Comparison distinguishes a whole’s retained name from its changing contents before asking whether the whole sequence repeats.",
      "cite": [
        {"w":"heraclitus-book","loc":"B12"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
      ]
    },
    "grounds": [
      {"ref":"heraclitus-river","cite":[{"w":"heraclitus-book","loc":"B12"}]},
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
        ]
      }
    ],
    "question": {
      "ru": "Какой именно предмет остаётся тем же в каждом из двух мест?",
      "en": "What exactly remains the same in each passage?"
    }
  },
  "atlas:heraclitus:questions:circle-change": {
    "warrant": {
      "ru": "Соразмерность перемен и повторение их полного хода открывают сравнительный вопрос о порядке становления.",
      "en": "Change’s measured order and the recurrence of its full course open a comparison about becoming’s order.",
      "cite": [
        {"w":"heraclitus-book","loc":"B30"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {"ref":"heraclitus-cosmos","cite":[{"w":"heraclitus-book","loc":"B30"}]},
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ],
    "question": {
      "ru": "Что нужно добавить к мерам изменения, чтобы получить тождественное повторение целого?",
      "en": "What must be added to change’s measures to obtain the whole’s identical recurrence?"
    }
  },
  "atlas:stoics:develops:providence": {
    "warrant": {
      "ru": "Доверие к порядку здесь развёрнуто через условные доводы.",
      "en": "Trust in order unfolds through conditional arguments.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
      ]
    },
    "grounds": [
      {
        "ref": "marcus6-44",
        "cite": [
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется основание действия при переходе от попечения богов к собственной природе?",
      "en": "How does action’s ground change from divine care to one’s own nature?"
    }
  },
  "atlas:stoics:develops:epictetus": {
    "warrant": {
      "ru": "Вход в стоическую ветвь дан различением подвластного и неподвластного и последствиями его смешения, которые текст сразу перечисляет.",
      "en": "The distinction between what is up to us and what is not, and the consequences of confusing its sides, which the text lists right away, provide an entrance into the Stoic branch.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":212}
      ]
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":721},
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      }
    ]
  },
  "atlas:epictetus:develops:agency": {
    "warrant": {
      "ru": "Перечень подвластного уточняет действующее начало изнутри: вопрос касается того, с чем человек соглашается и к чему направляется, прежде чем оценивать достигнутый внешний результат.",
      "en": "The list of what is up to us specifies agency from within: what one assents to and moves toward comes into question before the achieved external outcome.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":212}
      ]
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":212},
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется описание поступка, если начать с желания и суждения, а не с результата?",
      "en": "How does an action’s description change when it starts from desire and judgement rather than outcome?"
    }
  },
  "atlas:epictetus:compares:amor-fati": {
    "warrant": {
      "ru": "Оба места меняют отношение к происходящему, но делают это через разные действия мысли: различение и преобразование взгляда.",
      "en": "Both change a relation to events through different acts of thought: discrimination and transformed vision.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":329,"e":721},
        {"w":"die-froehliche-wissenschaft","loc":"276"}
      ]
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":329,"e":721},
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      },
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}
    ],
    "question": {
      "ru": "Совпадает ли освобождение от обвинения в этих двух ходах?",
      "en": "Does release from accusation mean the same thing in these two movements?"
    }
  },
  "atlas:providence:compares:necessity": {
    "warrant": {
      "ru": "Переход к порядку мира получает в этих местах разные основания.",
      "en": "World-order receives different grounds in these passages.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":2,"e":420},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
      ]
    },
    "grounds": [
      {
        "ref": "marcus6-44",
        "cite": [
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":2,"e":420},
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
        ]
      },
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется, когда необходимое объясняется благом, а не связью событий?",
      "en": "What changes when necessity is explained through goodness rather than events’ connection?"
    }
  },
  "atlas:providence:compares:cosmological": {
    "warrant": {
      "ru": "Сравнение разделяет смысл порядка для участника и форму повторения порядка.",
      "en": "Comparison distinguishes order’s meaning for a participant from the form of its recurrence.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":260,"e":420},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
      ]
    },
    "grounds": [
      {
        "ref": "marcus6-44",
        "cite": [
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":260,"e":420},
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47"]}
        ]
      }
    ],
    "question": {
      "ru": "Какой из двух вопросов остаётся, даже если на другой уже дан ответ?",
      "en": "Which question remains even after the other has been answered?"
    }
  },
  "atlas:spinoza:develops:conatus": {
    "warrant": {
      "ru": "Речь шире сознательного желания человека выжить.",
      "en": "The proposition extends beyond a human’s conscious wish to survive.",
      "cite": [{"w":"ethica","loc":"III.P6"}]
    },
    "grounds": [{"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]}],
    "question": {
      "ru": "Что означает «насколько от неё зависит» для вещи, окружённой другими причинами?",
      "en": "What does “as far as it can by its own power” mean for a thing surrounded by other causes?"
    }
  },
  "atlas:spinoza:compares:necessity": {
    "warrant": {
      "ru": "Точность определения мешает заранее отождествить необходимое с вынужденным.",
      "en": "The definition’s precision prevents an initial equation of necessity with compulsion.",
      "cite": [{"w":"ethica","loc":"I.D7"}]
    },
    "grounds": [{"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}],
    "question": {
      "ru": "Как изменится вопрос о свободе, если спрашивать об источнике определения, а не об отсутствии необходимости?",
      "en": "How does the question of freedom change if it concerns determination’s source rather than necessity’s absence?"
    }
  },
  "atlas:conatus:compares:affirmation": {
    "warrant": {
      "ru": "Сравнение соединяет существование и оценку, сохраняя вопрос, что должно добавиться к первому для второго.",
      "en": "Comparison connects existence and evaluation while asking what the latter requires beyond the former.",
      "cite": [{"w":"ethica","loc":"III.P6"},{"w":"die-froehliche-wissenschaft","loc":"276"}]
    },
    "grounds": [
      {"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]},
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}
    ],
    "question": {
      "ru": "Какое преобразование отношения к жизни описывает §276 сверх самого стремления продолжать существовать?",
      "en": "What transformation of one’s relation to life does §276 describe beyond striving to continue existing?"
    }
  },
  "atlas:conatus:compares:recurrence": {
    "warrant": {
      "ru": "Вопрос о продолжении вещи и вопрос о повторении её жизни имеют здесь разные предметы.",
      "en": "Continuing a thing and repeating its life have different objects here.",
      "cite": [
        {"w":"ethica","loc":"III.P6"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"]}
      ]
    },
    "grounds": [
      {"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]},
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"],"s":62},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-ce045055585467cfc09217fef32bfa81","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Что пришлось бы сохранить через смерть, чтобы продолжение и возвращение совпали?",
      "en": "What would have to be preserved through death for continuation and recurrence to coincide?"
    }
  },
  "atlas:leibniz:develops:possible-worlds": {
    "warrant": {
      "ru": "Возможность здесь включена в определённое объяснение действительного.",
      "en": "Possibility is here placed inside a definite explanation of actuality.",
      "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"}]
    },
    "grounds": [
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
      }
    ],
    "question": {
      "ru": "Почему множество возможностей усиливает, а не устраняет требование основания этого мира?",
      "en": "Why does a plurality of possibilities intensify rather than remove the demand for this world’s reason?"
    }
  },
  "atlas:possible-worlds:questions:identity": {
    "warrant": {
      "ru": "Сопоставление выявляет разницу между выбором целого и точностью его повтора.",
      "en": "Comparison brings out the difference between selecting a whole and repeating it exactly.",
      "cite": [
        {"w":"monadologie","loc":"54"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
      ]
    },
    "grounds": [
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему улучшение, служащее основанием выбора в одном месте, разрушило бы тождество в другом?",
      "en": "Why would improvement, grounding selection in one passage, break identity in the other?"
    }
  },
  "atlas:possible-worlds:compares:same-new": {
    "warrant": {
      "ru": "Сравнение уточняет два масштаба «иначе»: другое возможное целое и изменение внутри предполагаемого повтора.",
      "en": "Comparison specifies two scales of “otherwise”: another possible whole and an alteration within the proposed recurrence.",
      "cite": [{"w":"monadologie","loc":"53"},{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [
      {"ref":"leibniz53-55","cite":[{"w":"monadologie","loc":"53"}]},
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Что именно меняется при воображении другого мира, чего не допускает демон?",
      "en": "What changes when another world is imagined that the demon’s proposal excludes?"
    }
  },
  "atlas:possible-worlds:compares:amor-fati": {
    "warrant": {
      "ru": "Переход от сравнения возможных миров к изменению собственного взгляда меняет того, кто выбирает, и сам предмет выбора.",
      "en": "Moving from comparing possible worlds to changing one’s own vision changes both the chooser and the object of choice.",
      "cite": [
        {"w":"monadologie","loc":"54"},
        {"w":"monadologie","loc":"55"},
        {"w":"die-froehliche-wissenschaft","loc":"276"}
      ]
    },
    "grounds": [
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"},{"w":"monadologie","loc":"53"}]
      },
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}
    ],
    "question": {
      "ru": "Означает ли желание видеть этот мир прекрасным утверждение, что лучший мир невозможен?",
      "en": "Does wanting to see this world as beautiful assert that no better world is possible?"
    }
  },
  "atlas:kierkegaard:develops:repetition": {
    "warrant": {
      "ru": "Вопрос о повторении получает здесь одновременно логическую постановку и жест испытания.",
      "en": "Here the question of repetition receives both a conceptual formulation and a gesture of trial.",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    },
    "grounds": [{"ref":"kierkegaard-repetition","cite":[{"w":"gjentagelsen","loc":"opening"}]}],
    "question": {
      "ru": "Почему рассуждение о повторении начинается с действия, отвечающего на довод?",
      "en": "Why does reflection on repetition begin with an action answering an argument?"
    }
  },
  "atlas:repetition:contrasts:memory": {
    "warrant": {
      "ru": "Сначала объявленное родство повторения и воспоминания делает различие трудным — нужно понять, как уже бывшее может быть направлено вперёд.",
      "en": "The announced kinship of repetition and recollection makes the difference difficult: how can what has been acquire a forward direction?",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    },
    "grounds": [{"ref":"kierkegaard-repetition","cite":[{"w":"gjentagelsen","loc":"opening"}]}],
    "question": {
      "ru": "Что в повторении должно возобновиться, чтобы оно двигалось вперёд?",
      "en": "What must be renewed for repetition to move forward?"
    }
  },
  "atlas:repetition:compares:recurrence": {
    "warrant": {
      "ru": "Оба места спрашивают о «снова», но одно начинает с направления движения, другое — с полного содержания повторяемого.",
      "en": "Both ask about “again,” but one begins from movement’s direction and the other from recurrence’s full contents.",
      "cite": [{"w":"gjentagelsen","loc":"opening"},{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [
      {"ref":"kierkegaard-repetition","cite":[{"w":"gjentagelsen","loc":"opening"}]},
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Можно ли желать повторения вперёд, не желая точного возвращения всей жизни?",
      "en": "Can repetition forward be wanted without wanting an entire life’s exact return?"
    }
  },
  "atlas:kierkegaard:compares:existential": {
    "warrant": {
      "ru": "В обоих случаях понятие затрагивает существование спрашивающего, хотя объём обещанного «снова» ещё нужно различить.",
      "en": "Both concepts bear on the questioner’s existence, while the scope of “again” still needs distinguishing.",
      "cite": [{"w":"gjentagelsen","loc":"opening"},{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [
      {"ref":"kierkegaard-repetition","cite":[{"w":"gjentagelsen","loc":"opening"}]},
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Как вопрос о возможности превращается в вопрос о том, как человек живёт?",
      "en": "How does a question about possibility become a question of how a person lives?"
    }
  },
  "atlas:camus:interprets:sisyphus": {
    "warrant": {"cite":[{"w":"le-mythe-de-sisyphe","loc":"return"},{"w":"le-mythe-de-sisyphe","loc":"tragic"}]},
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [{"w":"le-mythe-de-sisyphe","loc":"return"},{"w":"le-mythe-de-sisyphe","loc":"whole"}]
      }
    ],
    "question": {
      "ru": "Почему решающее место рассуждения находится между повторяемыми усилиями?",
      "en": "Why does the decisive moment of the argument fall between repeated efforts?"
    }
  },
  "atlas:sisyphus:compares:wheel": {
    "warrant": {
      "ru": "Сходство повторяемого движения позволяет увидеть добавление у Камю: возвращение к началу включает осознание своего удела в паузе спуска.",
      "en": "Repeated movement’s resemblance reveals Camus’s addition: returning to the beginning includes awareness of one’s lot during the descent’s pause.",
      "cite": [
        {"w":"le-mythe-de-sisyphe","loc":"ascent"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"],"e":61},
        {"w":"le-mythe-de-sisyphe","loc":"return"}
      ]
    },
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [
          {"w":"le-mythe-de-sisyphe","loc":"ascent"},
          {"w":"le-mythe-de-sisyphe","loc":"return"},
          {"w":"le-mythe-de-sisyphe","loc":"whole"}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39"],"e":61}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется в круге, когда его участник знает о предстоящем возвращении?",
      "en": "What changes in the cycle when its participant knows of the coming return?"
    }
  },
  "atlas:camus:compares:affirmation": {
    "warrant": {
      "ru": "Сравнение исследует, как согласие становится задачей, оставаясь в разных постановках.",
      "en": "Comparison examines how assent becomes a task within differing formulations.",
      "cite": [{"w":"le-mythe-de-sisyphe","loc":"joy"},{"w":"die-froehliche-wissenschaft","loc":"276"}]
    },
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [{"w":"le-mythe-de-sisyphe","loc":"joy"},{"w":"le-mythe-de-sisyphe","loc":"last"}]
      },
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}
    ],
    "question": {
      "ru": "Какую работу совершает ясность у Камю и учение взгляда в §276?",
      "en": "What work is done by lucidity in Camus and learning to see in §276?"
    }
  },
  "atlas:sisyphus:develops:teach-live": {
    "warrant": {
      "ru": "Рядом с учительством Заратустры сознание Сизифа во время спуска делает различимым переход от произносимого знания к способу жить с ним: глава о выздоровлении тоже помещает речь после испытания.",
      "en": "Beside Zarathustra’s teaching, Sisyphus’s consciousness during the descent distinguishes articulated knowledge from a way of living with it: the convalescence chapter likewise places speech after an ordeal.",
      "cite": [
        {"w":"le-mythe-de-sisyphe","loc":"tragic"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
      ]
    },
    "grounds": [
      {"ref":"camus-sisyphus","cite":[{"w":"le-mythe-de-sisyphe","loc":"return"}]},
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c"]}
        ]
      }
    ],
    "question": {
      "ru": "Что испытание добавляет к знанию повторяемого условия?",
      "en": "What does undergoing the ordeal add to knowing the repeated condition?"
    }
  },
  "atlas:deleuze:develops:return-difference": {
    "warrant": {
      "ru": "«Различие» становится частью объяснения возвращения, а не только названием меняющихся состояний.",
      "en": "“Difference” enters the explanation of recurrence rather than merely naming changing states.",
      "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
    },
    "grounds": [
      {
        "ref": "deleuze-nietzsche-return",
        "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
      }
    ],
    "question": {
      "ru": "Как объяснение, начинающееся с различия, должно получить повторение?",
      "en": "How must an explanation beginning from difference arrive at recurrence?"
    }
  },
  "atlas:return-difference:interprets:difference": {
    "warrant": {
      "ru": "Ход Делёза требует объяснить, как различающее отношение образует синтез возвращения.",
      "en": "Deleuze’s move requires explaining how a differential relation forms recurrence’s synthesis.",
      "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
    },
    "grounds": [{"ref":"deleuze-nietzsche-return","cite":[{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]}],
    "question": {
      "ru": "Что в таком чтении должно вернуться: состояние сил или способ их различения?",
      "en": "What must return in this reading: a state of forces or the manner of their differentiation?"
    }
  },
  "atlas:return-difference:questions:identity": {
    "warrant": {
      "ru": "Сопоставление ставит точный вопрос интерпретации: как новое объяснительное начало прочитывает столь сильную формулу тождества?",
      "en": "Comparison poses a precise interpretive question: how does a new explanatory starting point read this strong formula of identity?",
      "cite": [
        {"w":"nietzsche-et-la-philosophie","loc":"II.5"},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-ce045055585467cfc09217fef32bfa81","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
      ]
    },
    "grounds": [
      {
        "ref": "deleuze-nietzsche-return",
        "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое значение «того же» сохраняется после делёзовского изменения основания?",
      "en": "Which meaning of “the same” survives Deleuze’s change of ground?"
    }
  },
  "atlas:deleuze:compares:becoming": {
    "warrant": {
      "ru": "Сравнительный переход ведёт от перечисления перемен к требованию объяснить их отношение.",
      "en": "Comparison moves from enumerating changes to demanding an explanation of their relation.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]},
        {"w":"nietzsche-et-la-philosophie","loc":"II.5"},
        {"w":"nietzsche-et-la-philosophie","loc":"II.6"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
        ]
      },
      {
        "ref": "deleuze-nietzsche-return",
        "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
      }
    ],
    "question": {
      "ru": "Что ещё нужно объяснить после слов «всё становится»?",
      "en": "What remains to be explained after saying that everything becomes?"
    }
  },
  "atlas:source-neighbor:supports:open-reading": {
    "warrant": {
      "ru": "Возвращение к соседним репликам открывает несколько связных прочтений этой последовательности, каждое из которых должно объяснить и песню, и необходимость следующего рассказа.",
      "en": "Returning to the neighbouring speeches opens several coherent readings of the sequence, each needing to explain both the song and the ensuing account’s necessity.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ],
    "question": {
      "ru": "Как соседняя реплика меняет то прочтение, которое казалось очевидным на одной формуле?",
      "en": "How does the neighbouring reply alter a reading that seemed obvious from one formula?"
    }
  },
  "atlas:poetic:interprets:dance": {
    "warrant": {
      "ru": "Танец участвует в способе желать вечности, а не только украшает уже готовый тезис.",
      "en": "Dance participates in how eternity is desired, beyond ornamenting a completed thesis.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-ab141acd46dabfd26bd179718bed791b","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-ab141acd46dabfd26bd179718bed791b","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"]}
        ]
      }
    ],
    "question": {
      "ru": "Какую перемену желания делает слышимой переход от речи к песне?",
      "en": "What change of desire becomes audible in the passage from speech to song?"
    }
  },
  "atlas:open-reading:develops:character-author": {
    "warrant": {
      "ru": "Несколько прочтений могут расходиться в значении молчания; его место в композиции даёт конкретное основание для сравнения атрибуций.",
      "en": "Readings may differ over the silence’s meaning; its compositional position provides a concrete basis for comparing attributions.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"],"s":170,"e":321}
        ]
      }
    ],
    "question": {
      "ru": "Подтверждает ли молчание эту речь, откладывает ответ или меняет сам предмет разговора?",
      "en": "Does the silence confirm the speech, defer a response or change the conversation’s object?"
    }
  },
  "atlas:agency:develops:decision": {
    "warrant": {
      "ru": "Решение можно исследовать между полученной возможностью и тем началом, ради которого она нужна.",
      "en": "Decision can be examined between an acquired possibility and the beginning for which it is needed.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]},
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"]}
        ]
      }
    ],
    "question": {
      "ru": "Когда возможность сказать «нет» превращается в возможность начать?",
      "en": "When does the ability to say no become the ability to begin?"
    }
  },
  "atlas:decision:develops:counterfactual": {
    "warrant": {
      "ru": "Вопрос «иначе» возникает между мысленным изменением поступка и уже совершившимся, которое воля не может сделать несовершившимся.",
      "en": "“Otherwise” arises between imagining an altered act and an accomplished past that willing cannot undo.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]}
        ]
      }
    ],
    "question": {
      "ru": "Что проверяет воображаемое «иначе», если оно уже не может изменить «было»?",
      "en": "What does imagining “otherwise” examine when it can no longer change what was?"
    }
  },
  "atlas:counterfactual:develops:responsibility": {
    "warrant": {
      "ru": "Переход к ответственности должен сначала исследовать это происхождение вменения: как невозможность отменить бывшее становится основанием обвинять?",
      "en": "Moving toward responsibility must first examine this origin of imputation: how does inability to undo what was become a ground for accusation?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-a602dadebebc6eb316a0caa49c4ca781","tos.text-unit.sid-a602dadebebc6eb316a0caa49c4ca781"],"s":85}
      ]
    },
    "grounds": [
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое отношение к прошлому предполагает обвинение, и что оно обещает воле?",
      "en": "What relation to the past does accusation presuppose, and what does it promise the will?"
    }
  },
  "atlas:responsibility:questions:affirmation": {
    "warrant": {
      "ru": "Сравнение открывает вопрос о смене отношения к бывшему: как мыслить собственное участие, когда обвинение перестаёт быть главным способом ответа?",
      "en": "Comparison opens a change of relation to what was: how can one’s own participation be thought when accusation ceases to be the principal response?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-a602dadebebc6eb316a0caa49c4ca781","tos.text-unit.sid-a602dadebebc6eb316a0caa49c4ca781"],"s":85},
        {"w":"die-froehliche-wissenschaft","loc":"276"}
      ]
    },
    "grounds": [
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3d1d58f8435846038b07514d30c5672a","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"]},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-ff240a868709abef0da385e34299d2c3"]}
        ]
      },
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}
    ],
    "question": {
      "ru": "Что становится предметом ответа после отказа от мести прошлому?",
      "en": "What becomes the object of response after revenge against the past is relinquished?"
    }
  },
  "atlas:nausea:develops:body-resistance": {
    "warrant": {
      "ru": "Телесная последовательность и позднее объяснение связывают мысль с тем, что герой способен вынести и произнести.",
      "en": "The bodily sequence and later account connect the thought with what he can bear and articulate.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4cf0a6689b0b424eaee11980c7fd6789","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4cf0a6689b0b424eaee11980c7fd6789","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему предмет болезни назван только после начала выздоровления?",
      "en": "Why is the illness’s object named only after recovery begins?"
    }
  },
  "atlas:dance:develops:lived-rhythm": {
    "warrant": {
      "ru": "Прожитый ритм можно исследовать в самом чтении того, как новый образ подводит к уже знакомым словам.",
      "en": "Lived rhythm can be examined in reading how a new image leads into already familiar words.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-745a62dbd7262ad9b8c0aa47c1262130","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
        ]
      }
    ],
    "question": {
      "ru": "Слышится ли тот же припев иначе после образа танцора?",
      "en": "Does the same refrain sound different after the dancer’s image?"
    }
  },
  "atlas:lived-rhythm:develops:return-practice": {
    "warrant": {
      "ru": "Текст сам даёт ограниченную практику повторного слушания. Сравнение двух вхождений припева помогает заметить, как сохранённые слова меняют отношение к предшествующему.",
      "en": "The text itself offers a bounded practice of listening again. Comparing two occurrences of the refrain reveals how retained words change their relation to what precedes them.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-seven-seals",
        "focus": {
          "ru": "Припев после §1 и после §§6–7: те же слова в другом образном окружении.",
          "en": "The refrain after §1 and after §§6–7: the same words in another setting of images."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-745a62dbd7262ad9b8c0aa47c1262130","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      }
    ],
    "question": {
      "ru": "Что вы услышали во втором припеве благодаря пройденному между ними?",
      "en": "What did you hear in the second refrain because of what intervened?"
    }
  },
  "atlas:body-resistance:develops:return-practice": {
    "warrant": {
      "ru": "После рассказа об отвращении можно вновь прочитать начальный вызов глубинной мысли: теперь известен предмет испытания, которого начало ещё не называет. Возврат меняет понимание временного порядка главы и позволяет проверить, что читатель раньше достраивал сам.",
      "en": "After the account of disgust, the opening summons to the deepest thought can be read again: the ordeal’s object is now known, though the beginning leaves it unnamed. Returning changes one’s grasp of the chapter’s sequence and reveals what the reader had supplied earlier."
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Перечитать начало §1 после слов о малом человеке в §2.",
          "en": "Reread the beginning of §1 after the words about the small human in §2."
        }
      }
    ],
    "question": {
      "ru": "Какая начальная деталь приобретает смысл только после позднейшего объяснения?",
      "en": "Which opening detail gains meaning only after the later explanation?"
    }
  },
  "atlas:memory:develops:remembered-return": {
    "warrant": {
      "ru": "Возвращение здесь дано как связь двух переживаний, установленная рассказчиком; это отличается от формулы, по которой вся жизнь должна повториться.",
      "en": "Return appears here as a connection between two experiences established by the narrator; this differs from a formula requiring an entire life to repeat.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
        ]
      }
    ],
    "question": {
      "ru": "Что именно вернулось в узнавании: звук, сцена или прежнее чувство?",
      "en": "What returned in recognition: a sound, a scene or an earlier feeling?"
    }
  },
  "atlas:remembered-return:develops:attention": {
    "warrant": {
      "ru": "Узнавание не завершает внимание, а ведёт его дальше, от знакомого звука к новому видению.",
      "en": "Recognition does not complete attention but carries it onward, from a familiar sound to a new vision.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему узнавание звука порождает новый вопрос вместо готового ответа?",
      "en": "Why does recognising the sound generate a new question rather than a ready answer?"
    }
  },
  "atlas:attention:develops:shared-world": {
    "warrant": {
      "ru": "Внимание собирает общий мир через различие участников.",
      "en": "Attention gathers a shared world through its participants’ differences.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-731a9e34e7c14198ab956f27436becc4"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e9175aa652493debe3e712faff10a3df","tos.text-unit.sid-e9175aa652493debe3e712faff10a3df"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e9175aa652493debe3e712faff10a3df","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]}
        ]
      }
    ],
    "question": {
      "ru": "Как общий ход сцены распределяется между тем, что видит и делает каждый?",
      "en": "How is the scene’s common course distributed among what each participant sees and does?"
    }
  },
  "atlas:shared-world:questions:whole-part": {
    "warrant": {
      "ru": "Целое названо через конкретное окружение говорящего; вопрос об участниках позволяет понять, зачем тождество жизни требует именно этих связей.",
      "en": "The whole is named through the speaker’s particular surroundings; attending to participants asks why life’s identity requires these very relations.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Осталась бы жизнь той же, если сохранить говорящего, но заменить его спутников?",
      "en": "Would the life remain the same if the speaker were retained but the companions replaced?"
    }
  },
  "atlas:camus:interprets:fate": {
    "warrant": {
      "ru": "Взгляд на связность собственного пути возникает при сохраняющемся камне: сделанное и осознанное входят в судьбу вместе с тем, что человек не устранил.",
      "en": "The view of one’s path as connected arises while the stone remains: what is done and understood enters fate together with what the person has not removed.",
      "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"}]
    },
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [
          {"w":"le-mythe-de-sisyphe","loc":"subtle"},
          {"w":"le-mythe-de-sisyphe","loc":"joy"},
          {"w":"le-mythe-de-sisyphe","loc":"last"}
        ]
      }
    ],
    "question": {
      "ru": "Какое значение получает «моё», когда судьба состоит и из поступков, и из принятого удела?",
      "en": "What does “mine” mean when fate consists of both actions and an encountered lot?"
    }
  },
  "atlas:sisyphus:develops:agency": {
    "warrant": {
      "ru": "Участие Сизифа заключено и в телесном усилии, и в ясном знании своего удела.",
      "en": "Sisyphus’s participation consists in both bodily effort and lucid knowledge of his lot.",
      "cite": [
        {"w":"le-mythe-de-sisyphe","loc":"subtle"},
        {"w":"le-mythe-de-sisyphe","loc":"last"},
        {"w":"le-mythe-de-sisyphe","loc":"return"}
      ]
    },
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [{"w":"le-mythe-de-sisyphe","loc":"return"},{"w":"le-mythe-de-sisyphe","loc":"subtle"}]
      }
    ],
    "question": {
      "ru": "Почему осознание собственного удела само меняет отношение к следующему усилию?",
      "en": "Why does awareness of one’s lot itself alter the relation to the next effort?"
    }
  },
  "atlas:responsibility:develops:creation": {
    "warrant": {
      "ru": "Связь ответственности с созиданием исследует эту смену ответа бывшему; её предметом становится отношение воли к своему участию в жизни.",
      "en": "Connecting responsibility with creation examines this changed response to what was: willing’s relation to its own participation in life.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3d1d58f8435846038b07514d30c5672a","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-351ef082d5a4c98dcd346a2cbfc1357f"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3d1d58f8435846038b07514d30c5672a","tos.text-unit.sid-351ef082d5a4c98dcd346a2cbfc1357f"]}
        ]
      }
    ],
    "question": {
      "ru": "Как созидание могло бы изменить ответ прошлому, которое оно не может отменить?",
      "en": "How might creation alter the response to a past it cannot undo?"
    },
    "limit": {
      "ru": "Глава ставит вопрос о вменении и избавлении через драму воли; общей теории ответственности она здесь не излагает.",
      "en": "The chapter approaches imputation and redemption through willing’s drama; it does not set out a general theory of responsibility here."
    }
  },
  "atlas:creation:develops:fate": {
    "warrant": {
      "ru": "Созидание позволяет исследовать судьбу через хотение становления: воля хочет такой судьбы, а не только узнаёт уже названный удел.",
      "en": "Creation allows fate to be examined through willing becoming: the will wants such a fate beyond recognising an already named lot.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      }
    ],
    "question": {
      "ru": "Как желание становления меняет смысл слова «судьба»?",
      "en": "How does desiring becoming change the meaning of “fate”?"
    }
  },
  "atlas:agency:develops:fate": {
    "warrant": {
      "ru": "Эти места дают два способа включить действующего в судьбу: через причинную связь и через собранную связь собственной жизни.",
      "en": "These passages include the agent in fate through two relations: causal connection and one’s life gathered as connected.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"],"s":105},
        {"w":"le-mythe-de-sisyphe","loc":"subtle"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"]}
        ]
      },
      {
        "ref": "camus-sisyphus",
        "cite": [
          {"w":"le-mythe-de-sisyphe","loc":"subtle"},
          {"w":"le-mythe-de-sisyphe","loc":"joy"},
          {"w":"le-mythe-de-sisyphe","loc":"last"}
        ]
      }
    ],
    "question": {
      "ru": "Что значит участвовать в судьбе в каждом из этих двух описаний?",
      "en": "What does participating in fate mean in each account?"
    }
  },
  "atlas:joy-pain:develops:shared-world": {
    "warrant": {
      "ru": "Переход делает возможным вопрос о том, как всеобщее утверждение звучит внутри отдельного отношения.",
      "en": "The transition asks how universal affirmation sounds within a particular relation.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]},
        {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-6d5d64df7654526f06be7538126618e8"]},
        {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a","tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860","tos.text-unit.sid-0412cd53b6bfbe92e402ee6a62bfb860"]}
        ]
      },
      {
        "ref": "z-pitiful",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-e265be83aa60fd32629dc350aa3c9ebc"]},
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a","tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a"]}
        ]
      }
    ],
    "question": {
      "ru": "Как общее «да» получает определённость в обращении к страдающему другу?",
      "en": "How does a general yes become definite in addressing a suffering friend?"
    }
  },
  "atlas:shared-world:develops:responsibility": {
    "warrant": {
      "ru": "Вопрос об ответе другому рождается из этой сложности самой встречи: мало назвать себя помогающим, нужно понять действие собственного дара.",
      "en": "Answering another becomes a question through the encounter’s own complexity: calling oneself helpful does not yet explain one’s gift’s action.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-e265be83aa60fd32629dc350aa3c9ebc","tos.text-unit.sid-6d5d64df7654526f06be7538126618e8"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-pitiful",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-09b9bca89edae4834da231d217a4db61"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется понимание своего поступка, если дар оказывается тяжёлым для принимающего?",
      "en": "How does understanding one’s action change when a gift burdens its recipient?"
    }
  },
  "atlas:spinoza:compares:leibniz": {
    "warrant": {
      "ru": "Сравнение уточняет, какую работу выполняет основание: определяет действие из природы или объясняет выбор целого.",
      "en": "Comparison specifies what a ground does: determine action from nature or explain a whole’s selection.",
      "cite": [
        {"w":"ethica","loc":"I.D7"},
        {"w":"monadologie","loc":"53"},
        {"w":"monadologie","loc":"54"},
        {"w":"monadologie","loc":"55"}
      ]
    },
    "grounds": [
      {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]},
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
      }
    ],
    "question": {
      "ru": "В каком отношении выбор может быть обоснованным и необходимым в каждом из этих ходов?",
      "en": "In what respect can choice be grounded and necessary in each of these movements?"
    }
  },
  "atlas:camus:develops:memory": {
    "warrant": {
      "ru": "Вопрос о созданном пути у Камю неотделим от вопроса о его обозрении.",
      "en": "In Camus, a made path is inseparable from surveying it.",
      "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"}]
    },
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [
          {"w":"le-mythe-de-sisyphe","loc":"subtle"},
          {"w":"le-mythe-de-sisyphe","loc":"joy"},
          {"w":"le-mythe-de-sisyphe","loc":"last"}
        ]
      }
    ],
    "question": {
      "ru": "Что память соединяет в жизни и что получает границу лишь благодаря смерти?",
      "en": "What does memory join in a life, and what gains a boundary only through death?"
    }
  }
};
