/** Authored close inquiry of the readings and echoes; grounds name exact places.
 * Comparisons are argued here, without changing source or canon status.
 * Own words are kept only for what a quotation cannot give (a reading, a
 * reconstructed argument, a distinction, a connection, a question, a method,
 * context). What a work itself says is never retold: a form cites the place
 * instead ({ru, en, cite}), and the stand shows the work's own text there —
 * the original and, through the alignment of the witnesses, the reader's
 * language (bonsai/zarathustra/witnesses.json, fragments.json). */
export const INQUIRY_READINGS_ECHOES = {
  "demo:spinoza": {
    "argument": {
      "ru": "Поэтому отсутствие причин не входит в определение свободы: решающей становится необходимость собственной природы. Этот ход меняет сам предмет спора о свободе — с возможности произвольно прервать порядок на способ определения действующего.",
      "en": "An absence of causes therefore does not belong to the definition of freedom: the necessity of one’s own nature is decisive. This changes the subject of the dispute about freedom from arbitrarily interrupting an order to the manner in which the agent is determined.",
      "cite": [{"w":"ethica","loc":"I.D7"}]
    },
    "grounds": [{"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}],
    "question": {
      "ru": "Почему Спинозе требуется уточнение «или, лучше, принуждённое», если необходимость уже присутствует в определении свободного?",
      "en": "Why does Spinoza need the qualification “or rather constrained” when necessity already appears in the definition of freedom?"
    }
  },
  "demo:epictetus": {
    "argument": {
      "ru": "В первом разделе «Энхиридиона» граница проходит не между большим и малым влиянием, а между собственными суждениями, стремлениями и тем, что не принадлежит собственной деятельности. Обещанная свобода от принуждения поэтому связана с переменой того, что человек считает своим, а не с усилением власти над обстоятельствами.",
      "en": "In the Enchiridion’s first section, the boundary runs not between greater and lesser influence but between one’s own judgments and impulses and what does not belong to one’s own activity. The promised freedom from constraint is therefore connected with changing what one regards as one’s own rather than increasing power over circumstances.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":130,"e":177},
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":867,"e":1066},
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":1067,"e":1379}
      ]
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      }
    ],
    "question": {
      "ru": "Что изменилось бы в доводе, если заменить «принадлежит нам» на «поддаётся нашему влиянию»?",
      "en": "What would change in the argument if “belongs to us” were replaced with “can be influenced by us”?"
    }
  },
  "demo:joy-pain": {
    "argument": {
      "ru": "Здесь боль не выступает заранее установленной последней мерой ценности: требуется понять сам переход от отвращения к утверждению. Главы «О сострадательных» и «Знамение» уточняют эту трудность: создающая любовь ставится выше жалости, а сострадание к высшим людям оказывается последним грехом Заратустры перед возвращением к своему делу. Чтение этих мест спрашивает, каким образом меняется оценка страдания внутри движения книги.",
      "en": "Pain is not an already established final measure of value here: the transition from nausea to affirmation itself needs understanding. “The Pitiful” and “The Sign” sharpen this difficulty: creating love is placed above pity, while fellow-suffering with the higher men becomes Zarathustra’s last sin before returning to his work. Reading these passages asks how suffering’s evaluation changes within the book’s movement.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-8a03719e2423c8c51817085f23002bbf"],"s":109},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":1},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-a296affd8de2364380938f345fdc44b6","tos.text-unit.sid-c6fa872b1e91669e6786d92f635bed5d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-8a03719e2423c8c51817085f23002bbf"],"s":109},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":1}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-a296affd8de2364380938f345fdc44b6","tos.text-unit.sid-84a49cb92d6a379740a925cb522d2792"]}
        ]
      },
      {
        "ref": "z-pitiful",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a","tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a"],"s":0,"e":153},
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c","tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c"],"s":29,"e":124}
        ]
      },
      {
        "ref": "z-sign",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-c0e3a3517b021107cee042466f68a20d","tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae"],"s":125}
        ]
      }
    ],
    "question": {
      "ru": "Что связывает отвращение к возвращению малого с преодолением сострадания к высшим людям — один предмет оценки или изменение того, ради чего Заратустра действует?",
      "en": "What connects nausea at the small human’s return with overcoming pity for the higher men: one object of evaluation, or a change in what Zarathustra acts for?"
    }
  },
  "demo:cosmological": {
    "argument": {
      "ru": "Переход от бесконечного времени к повторению требует отдельного довода: последовательность может быть бесконечной в обоих направлениях и состоять из разных состояний. Ряд всех целых чисел показывает эту логическую возможность, сохраняя две неограниченные дороги вокруг выбранного нуля. Он позволяет точно выделить недостающий переход от бесконечности к возвращению, не выдавая условную модель за описание мира.",
      "en": "Moving from infinite time to repetition needs a separate argument: a sequence can be infinite in both directions and consist of different states. The sequence of all integers shows this logical possibility while preserving two unbounded paths around a chosen zero. It isolates the missing transition from infinity to recurrence without presenting a hypothetical model as a description of the world.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Две бесконечные дороги и вопросы о том, что может и должно произойти, служат предметом логической реконструкции.",
          "en": "Two infinite paths and questions about what can and must happen are the object of logical reconstruction."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-317301cd82d35cd0e2d4362b72783488","tos.text-unit.sid-43924bb3e021a1d1fb27927e2f4b02f0"]}
        ]
      }
    ],
    "question": {
      "ru": "Какая дополнительная посылка превращает бесконечность дороги в повторение её полного хода?",
      "en": "Which additional premise turns the path’s infinity into repetition of its complete course?"
    },
    "experiment": {
      "setup": {
        "ru": "Рассмотрите условный ряд …, −2, −1, 0, 1, 2, … . От нуля он неограниченно продолжается в обе стороны, и каждое число встречается один раз.",
        "en": "Consider the hypothetical sequence …, −2, −1, 0, 1, 2, … . From zero it extends indefinitely in both directions, and each number occurs once."
      },
      "question": {
        "ru": "Какое условие, кроме бесконечности обеих дорог, должно исключить такую последовательность из довода о возвращении?",
        "en": "Which condition besides the infinity of both paths must exclude such a sequence from an argument for recurrence?"
      }
    }
  },
  "demo:existential": {
    "argument": {
      "ru": "Экзистенциальное прочтение прослеживает этот переход от сообщения о мире к преобразованию желания, сохраняя радикальность последнего вопроса — захотеть только такого окончательного подтверждения жизни.",
      "en": "An existential reading follows this transition from a statement about the world to a transformation of desire, preserving the final question’s radical demand: to want nothing but this ultimate confirmation of life.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}],
    "question": {
      "ru": "Как меняется предмет §341 между первым сообщением демона и последним вопросом к адресату?",
      "en": "How does §341’s subject change between the demon’s initial announcement and the final question to the addressee?"
    }
  },
  "demo:poetic": {
    "argument": {
      "ru": "После вопросов о времени сцена пастуха переносит мысль из разговора в телесное событие. Такое устройство удерживает загадку внутри действия: различие неудавшейся помощи и собственного укуса пастуха позволяет читать перемену как нечто большее, чем получение объяснения.",
      "en": "After the questions about time, the shepherd’s scene moves thought from conversation into a bodily event. This arrangement holds the riddle within the action: the difference between failed assistance and the shepherd’s own bite allows the change to be read as more than receiving an explanation.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"],"s":0},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":2}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":0}
        ]
      }
    ],
    "question": {
      "ru": "Какую часть этой последовательности теряет толкование, сводящее укус к простому получению верного тезиса?",
      "en": "Which part of this sequence is lost by interpreting the bite simply as receiving a correct thesis?"
    }
  },
  "demo:temporal-knot": {
    "argument": {
      "ru": "Сопоставление различает связанность событий и работу воли над бывшим: образ узла не решает за неё задачу избавления. Поэтому общий вопрос о времени расходится на вопрос о ходе мира и вопрос о значении «я хотел этого».",
      "en": "Comparison distinguishes events’ connectedness from the will’s work upon the past; the knot’s image does not accomplish redemption for it. The shared question of time thus divides into the world’s course and the meaning of “I willed this”.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
        ]
      },
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-5d7585f48e3c3ff7eef88bb204463cc3","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-ff240a868709abef0da385e34299d2c3"],"s":0}
        ]
      }
    ],
    "question": {
      "ru": "Что объясняет связанность событий и чего она ещё не объясняет в требовании волить бывшее?",
      "en": "What does events’ connectedness explain, and what does it leave unexplained in the demand to will what has been?"
    }
  },
  "demo:voice-reading": {
    "argument": {
      "ru": "Первое лицо внутри предположения зверей поэтому имеет два уровня: говорящих зверей и предполагаемого говорящего Заратустру. Такое устройство позволяет различать изложение учения, присвоение голоса учителя и состояние самого учителя, не устраняя содержания сказанного.",
      "en": "The first person within the animals’ supposition therefore has two levels: the animals speaking and Zarathustra as the imagined speaker. This arrangement distinguishes an exposition of the teaching, an assumption of the teacher’s voice and the teacher’s own condition without removing what is said.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2"],"s":0},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"],"s":120,"e":320}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется в атрибуции формулы той же жизни, когда первое лицо прочитано внутри предположения зверей?",
      "en": "What changes in attributing the same-life formula when its first person is read within the animals’ supposition?"
    }
  },
  "demo:performative": {
    "argument": {
      "ru": "§341 различает содержание мысли и её власть над тем, кто её мыслит. Поэтому её действие не сводится к перемене настроения — меняется вес, который приобретает поступок в отношении целой жизни.",
      "en": "Section 341 distinguishes the thought’s content from its power over the thinker. Its effect therefore exceeds a change of mood—the weight an action acquires in relation to the whole life changes.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}],
    "question": {
      "ru": "Почему неизменность условия демона совместима с противоположными ответами и что тогда становится предметом испытания?",
      "en": "Why is the demon’s unchanging condition compatible with opposite responses, and what then becomes the object of the test?"
    }
  },
  "demo:selective": {
    "argument": {
      "ru": "Выбор счастливого эпизода меняет не ответ на условие, поставленное в §341, а само условие. Сопоставление двух форм показывает переход от перечисления без исключений к образу единства, которое соединяет различное.",
      "en": "Selecting a happy episode changes not the response to the condition stated in §341 but the condition itself. Comparing these forms reveals a movement from enumeration without exceptions to an image of unity joining what differs.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-a296affd8de2364380938f345fdc44b6","tos.text-unit.sid-c6fa872b1e91669e6786d92f635bed5d"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-a296affd8de2364380938f345fdc44b6","tos.text-unit.sid-84a49cb92d6a379740a925cb522d2792"]}
        ]
      }
    ],
    "question": {
      "ru": "Что образ смеси добавляет к строгому перечислению и почему его нельзя заменить удалением неприятных составляющих?",
      "en": "What does the mixture image add to strict enumeration, and why can it not be replaced by removing unwelcome ingredients?"
    }
  },
  "demo:small-human": {
    "argument": {
      "ru": "Повтор малого означает, что жизнь не завершится однажды миром, в котором преодолённое навсегда исчезло. Трудность касается поэтому масштаба утверждения: способна ли мысль включить возвращение того, на преодоление чего направлено стремление?",
      "en": "The small human’s repetition means life will not culminate in a world from which what was overcome has vanished forever. The difficulty therefore concerns affirmation’s scope: can the thought include the return of what the striving aims to overcome?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-e5c2cd07214c95f19a1a4dcf30a074b1"],"s":109},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f44f3910a461757623937a1e557a79e8","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-e5c2cd07214c95f19a1a4dcf30a074b1"],"s":109},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":1}
        ]
      }
    ],
    "question": {
      "ru": "Почему вечное возвращение малого затрудняет не только оценку прошлого, но и замысел преодоления?",
      "en": "Why does the small human’s eternal recurrence trouble not only an evaluation of the past but the project of overcoming?"
    }
  },
  "demo:nonlinear": {
    "argument": {
      "ru": "Позднее место позволяет вернуться к раннему и связать видение с испытанием того, кто его рассказывал. Но это движение понимания не требует считать все детали первой сцены уже окончательно расшифрованными.",
      "en": "The later passage permits a return to the earlier one, connecting the vision with the ordeal of its narrator. Yet this movement of understanding does not require treating every detail of the first scene as definitively decoded.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-47ecea77ae8a30862d06ed7c80ebdf1a","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":0}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое отождествление теперь получает основание, а какие вопросы первой сцены остаются открытыми?",
      "en": "Which identification now gains grounds, and which questions of the first scene remain open?"
    }
  },
  "demo:form-content": {
    "argument": {
      "ru": "Речь у ворот движется вопросами, а не рядом завершённых доказательств. Если заменить вопросы уверенными утверждениями, исчезнет различие между предложенной необходимостью и её усвоением говорящим. Форма сцены показывает степень достигнутого понимания и путь, который мысль ещё должна пройти.",
      "en": "The gateway speech advances through questions rather than a series of completed proofs. Replacing questions with confident assertions removes the distinction between a proposed necessity and its appropriation by the speaker. The scene’s form shows the degree of understanding reached and the path thought still has to travel.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"],"s":1}
        ]
      }
    ],
    "question": {
      "ru": "Какие предпосылки пришлось бы явно доказать, если превратить вопросительную речь в теорему?",
      "en": "Which assumptions would need explicit proof if the interrogative speech were turned into a theorem?"
    }
  },
  "demo:task": {
    "argument": {
      "ru": "Между желанием одарить и назначением учителем возникает задача — не только произнести мысль, но и стать тем, чьё дело она определяет. Звери могут назвать эту задачу, однако молчание Заратустры оставляет её исполнение в движении книги.",
      "en": "Between the wish to bestow and the designation as teacher emerges a task—not only to state a thought but to become someone whose work it determines. The animals can name that task, while Zarathustra’s silence leaves its fulfilment within the book’s continuing movement.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"],"s":7},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c"],"s":80,"e":158}
      ]
    },
    "grounds": [
      {
        "ref": "z-prologue",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"],"s":7}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"],"s":80},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Чем различаются желание передать накопленное и принятие мысли как собственного дела?",
      "en": "How do the wish to transmit what has accumulated and taking a thought as one’s own work differ?"
    }
  },
  "demo:open-reading": {
    "argument": {
      "ru": "Упрёк зверям за шарманочный припев допускает разные вопросы: ошиблись ли они в содержании или сделали слишком лёгким способ его произнесения. Сопоставление прочтений становится содержательным, когда каждое объясняет эти места вместе. Тогда выбор зависит от того, как связаны формула, голос и переживание, а не от простого числа подтверждающих строк.",
      "en": "The rebuke to the animals for their barrel-organ refrain allows different questions: is their content mistaken, or have they made its utterance too easy? Comparing readings becomes substantive when each explains these passages together. Preference then depends on how formula, voice and experience are connected rather than on counting supporting lines.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":0},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"],"s":0}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":0},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое место изменяет ваш ответ на вопрос, чему именно возражает Заратустра в речи зверей?",
      "en": "Which passage changes your answer to what exactly Zarathustra objects to in the animals’ speech?"
    }
  },
  "demo:lived-rhythm": {
    "argument": {
      "ru": "Значит, различие проходит не просто между повтором и новизной или между речью и музыкой. Исследовать ритм здесь — значит выяснять, как повторяемая форма связана с состоянием того, кто её произносит и слышит.",
      "en": "The distinction therefore does not fall simply between repetition and novelty or between speech and music. Investigating rhythm here means asking how a repeated form is connected with the condition of its speaker and listener.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"],"s":3},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-1fdb6b338b3ca379b3fc0d4b9a295d84","tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e"],"s":1}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Повторяющаяся песня, упрёк в шарманочности и последующий совет новых песен для новых лир дают разные функции звучащего повтора.",
          "en": "The repeated song, barrel-organ rebuke and later advice of new songs for new lyres give sounding repetition different functions."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему песня может в одной части разговора упрощать мысль, а в другой сопровождать выздоровление?",
      "en": "Why can song simplify the thought in one part of the conversation and accompany convalescence in another?"
    }
  },
  "demo:return-practice": {
    "argument": {
      "ru": "Повторное чтение позволяет проверить связь двух сцен на неизменившихся словах. Если после «Выздоравливающего» пастух начинает напоминать самого Заратустру, основанием служит позднейший рассказ об укусе чудовища. Практика возвращения поэтому сопоставляет приобретённое основание с тем, что оно ещё не объяснило.",
      "en": "Rereading allows the connection between two scenes to be examined against unchanged words. If the shepherd begins to resemble Zarathustra himself after “The Convalescent”, the ground is the later account of biting the monster. The practice of returning therefore compares a newly acquired ground with what it has not yet explained.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"],"s":0},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-c1a61ec1014026941c54254a0978115d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":53}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-c1a61ec1014026941c54254a0978115d"]}
        ]
      }
    ],
    "experiment": {
      "setup": {
        "ru": "Прочитайте сцену пастуха до и после рассказа Заратустры о чудовище. Назовите одну связь, ставшую обоснованнее, и одну деталь, которая всё ещё требует объяснения.",
        "en": "Read the shepherd’s scene before and after Zarathustra’s account of the monster. Name one connection that has gained grounds and one detail that still requires explanation."
      },
      "question": {
        "ru": "Чем второе чтение изменило именно довод, а не только впечатление знакомости?",
        "en": "How did the second reading change the argument itself rather than merely the impression of familiarity?"
      }
    }
  },
  "demo:fatalism": {
    "argument": {
      "ru": "Вопрос о бездействии требует различить зависимость поступка от причин и отсутствие у него действия на происходящее. В доказательстве conatus единичная вещь определённым образом выражает силу, посредством которой Бог существует и действует; зависимость её природы не превращает её в отсутствие силы. Эти два входа по-разному сохраняют действие внутри ограничения, не обещая независимости от всего остального.",
      "en": "The question of inaction requires distinguishing an action’s dependence on causes from its having no effect on what happens. In the conatus proof, an individual thing expresses in a determinate way the power whereby God exists and acts; the dependence of its nature does not turn it into an absence of power. These entries preserve action within limitation in different ways without promising independence from everything else.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":130,"e":177},
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":1067,"e":1379}
      ]
    },
    "grounds": [
      {"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]},
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      }
    ],
    "question": {
      "ru": "На каком дополнительном шаге признание зависимости стало бы утверждением, что собственное действие ничего не меняет?",
      "en": "At which additional step would acknowledging dependence become the claim that one’s own activity changes nothing?"
    }
  },
  "demo:same-new": {
    "argument": {
      "ru": "Внутри описанной жизни встреча с демоном сама остаётся встречей, вызывающей ещё не предрешённый для адресата ответ. Это различает новизну события для переживающего и отличие одного полного прохождения от другого. Добавление памяти о предыдущем цикле изменило бы условие, потому что в повторе должна сохраняться и последовательность мыслей.",
      "en": "Within the described life, meeting the demon remains an encounter calling forth a response not already settled for the addressee. This distinguishes an event’s novelty for the person experiencing it from a difference between complete passages. Adding memory of a preceding cycle would change the condition, since the sequence of thoughts must repeat too.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}],
    "question": {
      "ru": "В каком смысле встреча может быть первой, если вся жизнь по условию уже повторяется?",
      "en": "In what sense can an encounter be first if the whole life is already repeating under the hypothesis?"
    }
  },
  "demo:circle-change": {
    "argument": {
      "ru": "Повтор полной последовательности не устраняет изменений внутри неё: повторяться может именно переход между различными состояниями. Ни один из двух фрагментов сам по себе ещё не задаёт полный цикл. Поэтому сравнение различает изменение, сохраняющуюся связь и точное повторение всех состояний.",
      "en": "Repeating a complete sequence does not remove changes within it: what repeats may be the transition between different states. Neither fragment alone yet specifies a complete cycle. Comparison therefore distinguishes change, a persisting connection and exact repetition of every state.",
      "cite": [{"w":"heraclitus-book","loc":"B12"},{"w":"heraclitus-book","loc":"B30"}]
    },
    "grounds": [
      {
        "ref": "heraclitus-river",
        "focus": {
          "ru": "Это основание различить сохранение и неподвижность.",
          "en": "This grounds a distinction between persistence and immobility."
        },
        "cite": [{"w":"heraclitus-book","loc":"B12"}]
      },
      {
        "ref": "heraclitus-cosmos",
        "focus": {
          "ru": "Полное возвращение состояния остаётся дополнительным вопросом.",
          "en": "Complete recurrence of a state remains an additional question."
        },
        "cite": [{"w":"heraclitus-book","loc":"B30"}]
      }
    ],
    "question": {
      "ru": "Какое условие превращает упорядоченную смену состояний именно в замкнутую последовательность?",
      "en": "Which condition turns an ordered succession of states into a closed sequence?"
    }
  },
  "demo:free-necessary": {
    "argument": {
      "ru": "Если назвать несвободным всё необходимое, первое предложение спинозовского определения станет непонятным: свободная вещь существует именно необходимо. Поэтому для этого определения важно не отсутствие основания действия, а то, достаточно ли собственной природы для его определения. Вопрос о свободе внутри необходимости начинается с восстановления этой более строгой границы.",
      "en": "If everything necessary is called unfree, the first sentence of Spinoza’s definition becomes unintelligible: the free thing exists necessarily. For this definition, what matters is therefore not the absence of grounds for action but whether its own nature suffices to determine it. The question of freedom within necessity begins by recovering this stricter boundary.",
      "cite": [{"w":"ethica","loc":"I.D7"}]
    },
    "grounds": [
      {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]},
      {
        "ref": "epictetus1",
        "focus": {
          "ru": "Отсюда начинается сопоставление двух определений.",
          "en": "Comparison of the two definitions begins here."
        },
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":0,"e":328}
        ]
      }
    ],
    "question": {
      "ru": "Что исключает выражение «собой одним», чего не исключало бы простое переживание собственного желания?",
      "en": "What does “by itself alone” exclude that merely experiencing one’s own desire would not?"
    }
  },
  "demo:teach-live": {
    "argument": {
      "ru": "После этого звери способны изложить возвращение легче, чем он способен о нём говорить. Разница между формулой и переживанием поэтому задана не обвинением учителя извне, а последовательностью самой главы. Название «выздоравливающий» удерживает понимание во времени: произнесённое учение ещё связано с преобразованием того, кто должен его нести.",
      "en": "Afterwards, the animals can expound recurrence more easily than he can speak of it. The difference between formula and experience is therefore given by the chapter’s own sequence rather than an external accusation against the teacher. The name “convalescent” keeps understanding within time: the stated teaching is still connected with the transformation of the one who must bear it.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-910e30f1208895e720462b0097068f7b","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"],"s":0},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca"],"s":0,"e":289}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-910e30f1208895e720462b0097068f7b","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"],"s":0},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca"],"s":0,"e":289},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":0},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"],"s":0}
        ]
      }
    ],
    "question": {
      "ru": "Что объясняет болезнь в движении главы, чего не объяснило бы простое незнание формулы?",
      "en": "What does illness explain in the chapter’s movement that merely not knowing the formula would not?"
    }
  },
  "demo:human-overcoming": {
    "argument": {
      "ru": "Во «Выздоравливающем» возвращение малого осложняет представление о преодолении как окончательном исчезновении прежнего. Вместе эти места ставят вопрос о том, что меняется в созидающем, если преодолённое продолжает принадлежать целому возвращающейся жизни.",
      "en": "In “The Convalescent”, the small human’s return complicates understanding overcoming as the final disappearance of what came before. Together these passages ask what changes in the creator if what has been overcome continues to belong to the whole of returning life.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]},
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"],"s":1}
        ]
      }
    ],
    "question": {
      "ru": "Почему завоевание свободы для созидания ещё не является самим созданием ценностей?",
      "en": "Why does winning freedom for creation not yet constitute creating values?"
    }
  },
  "demo:proposition-poem": {
    "argument": {
      "ru": "Космологический довод можно реконструировать из вопросов у ворот, но такая реконструкция объясняет только часть движения главы. Поэтическая форма не отменяет вопроса о предпосылках: она добавляет вопрос о том, как мысль становится испытанием. Поэтому тезис и сцена могут исследоваться вместе без превращения каждого образа в отдельное доказательство.",
      "en": "A cosmological argument can be reconstructed from the gateway questions, but that reconstruction explains only part of the chapter’s movement. Poetic form does not remove the question of assumptions: it adds the question of how a thought becomes an ordeal. Thesis and scene can therefore be examined together without making each image a separate proof.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"],"s":1,"e":149},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":53}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":53}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется задача читателя при переходе от ворот к пастуху?",
      "en": "How does the reader’s task change in the transition from the gateway to the shepherd?"
    }
  },
  "demo:character-author": {
    "argument": {
      "ru": "Поэтому позицию произведения нельзя восстановить одним изъятием торжественной формулы: сама смена речи становится предметом вопроса. Различение автора и персонажа здесь начинается с устроенного автором отношения голосов, а не с предположения, что ни один голос ничего не утверждает.",
      "en": "The work’s position therefore cannot be reconstructed simply by extracting a solemn formula: the change in speech itself becomes a question. Distinguishing author and character here begins with the author’s arrangement of voices, not the assumption that no voice asserts anything.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-b0d5af9e13780eeaefe97a07e4b10fc2","tos.text-unit.sid-eacfb2c146f965b9faf6bd963d67805e"],"s":0},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-77d2134e63b47a686837068668c73b22","tos.text-unit.sid-225401ec25fcfee3ede1e91e2605ec08"],"s":25},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-99c899ddf58551f0aed506e19f12fc69","tos.text-unit.sid-225401ec25fcfee3ede1e91e2605ec08"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-7c1ded9bb2862ae4e10daac0083b0c86","tos.text-unit.sid-b003bab9168e85ef8eea6629dabcd8ba"],"s":0},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-eacfb2c146f965b9faf6bd963d67805e","tos.text-unit.sid-eacfb2c146f965b9faf6bd963d67805e"]},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-99c899ddf58551f0aed506e19f12fc69","tos.text-unit.sid-225401ec25fcfee3ede1e91e2605ec08"]}
        ]
      }
    ],
    "question": {
      "ru": "Что позволяет объяснить смена адресата, чего не видно в отдельно выписанной формуле об избавлении?",
      "en": "What does the change of addressee explain that is invisible in an isolated formula about redemption?"
    }
  },
  "demo:again-memory": {
    "argument": {
      "ru": "Если во втором прохождении появляется дополнительное воспоминание о первом, последовательность мыслей уже отличается. Ощущение знакомости, напротив, могло бы входить в обе одинаковые жизни и потому не удостоверяло бы переход между ними. Так вопрос об узнавании отделяет условие полного повтора от возможного свидетельства о нём.",
      "en": "If an additional memory of the first passage appears in the second, the sequence of thoughts already differs. A feeling of familiarity, by contrast, could belong to both identical lives and would therefore not establish a passage between them. The question of recognition thus separates the condition of complete repetition from possible evidence of it.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [
      {
        "ref": "gs341",
        "focus": {
          "ru": "Рассуждение о памяти выводит следствие этого условия.",
          "en": "The discussion of memory draws out a consequence of that condition."
        },
        "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
      }
    ],
    "question": {
      "ru": "Как должна быть устроена память, чтобы сохраниться в строгом повторе и не добавить к нему нового события?",
      "en": "How must memory be constituted to remain within strict repetition without adding a new event to it?"
    }
  },
  "demo:whole-part": {
    "argument": {
      "ru": "В §341 та же полнота выражается через перечисление малого и великого в прежнем порядке. Сопоставление различает два довода о целом: связанность событий и условие тождества всей последовательности. Изменение одной подробности нарушает второе условие, но само по себе ещё не показывает, какие именно причинные связи нарушены в первом.",
      "en": "In §341, the same completeness is expressed by enumerating the small and great in their former order. Comparison distinguishes two arguments about a whole: events’ connectedness and the condition of an entire sequence’s identity. Changing one detail violates the second condition without yet showing which causal connections are broken in the first.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]},
        {"w":"die-froehliche-wissenschaft","loc":"341"}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Какой вид связи вы утверждаете, говоря, что без одного мгновения не было бы всей этой жизни?",
      "en": "Which kind of connection are you asserting when saying that without one moment this whole life would not exist?"
    }
  },
  "demo:cosmos-evidence": {
    "argument": {
      "ru": "Вопросы у ворот подводят к всеобщему утверждению о повторе, но не представляют собой наблюдение нескольких мировых циклов. Для оценки космологического прочтения важно различить основание, заданное моделью, и основание, полученное из наблюдения. Конечное число замеченных повторов не исключает дальнейшего отличия, как и сходство не устанавливает полного тождества. Поэтому исследование ищет посылку, которая связывает доступное описание со всеобщим выводом.",
      "en": "The gateway questions approach a universal claim about recurrence, but they are not an observation of several world cycles. Assessing a cosmological reading requires distinguishing grounds supplied by a model from grounds obtained through observation. Finitely many observed repetitions do not exclude a later difference, just as resemblance does not establish complete identity. Inquiry therefore seeks the premise connecting the available description with a universal conclusion."
    },
    "grounds": [
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Бесконечные дороги и вопросы о возможном/необходимом дают именно рассуждение для реконструкции, а не отчёт о наблюдавшихся циклах.",
          "en": "Infinite paths and questions about possibility and necessity provide reasoning to reconstruct rather than a report of observed cycles."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-317301cd82d35cd0e2d4362b72783488","tos.text-unit.sid-43924bb3e021a1d1fb27927e2f4b02f0"]}
        ]
      }
    ],
    "question": {
      "ru": "Что могло бы обосновать переход от сходных событий к возвращению всех событий в том же порядке?",
      "en": "What could ground the transition from similar events to all events returning in the same order?"
    }
  },
  "demo:decision": {
    "argument": {
      "ru": "Будущий повтор здесь не предлагается как новая попытка исправить результат: он сохраняет тот же поступок в прежнем ряду. Поэтому решение получает иной горизонт — желание его бесчисленного возвращения вместе с целой жизнью. Это позволяет исследовать действие через его связь с подтверждением жизни, не подменяя вопрос выбором улучшенной следующей версии.",
      "en": "Future recurrence is not offered as another attempt to correct the result: it retains the same act in the former sequence. A decision therefore acquires another horizon—the desire for its countless returns together with the whole life. This allows action to be examined through its connection with confirming life without replacing the question with choosing an improved next version.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
    },
    "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}],
    "question": {
      "ru": "Как меняется оценка поступка, если следующего возвращения нельзя использовать для его исправления?",
      "en": "How does evaluating an action change if the next return cannot be used to correct it?"
    }
  },
  "demo:responsibility": {
    "argument": {
      "ru": "Сопоставление исследует две причины обвиняющего отношения — ошибку в границе собственного и бессилие перед временем. Оно не даёт единой теории распределения вины, а позволяет точнее поставить вопрос, от какого отношения каждый текст ищет освобождения.",
      "en": "Comparison examines two sources of an accusatory relation—a mistake about the boundary of one’s own and powerlessness before time. It supplies no unified theory of allocating blame, but sharpens the question of which relation each text seeks release from.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":329,"e":468},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-bc8111269b7adf1e6f1a2c297ae8b00b","tos.text-unit.sid-e1c8595b210fbd3da5aa409da133ad01"],"s":0}
      ]
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":329,"e":634}
        ]
      },
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-bc8111269b7adf1e6f1a2c297ae8b00b","tos.text-unit.sid-e1c8595b210fbd3da5aa409da133ad01"],"s":0}
        ]
      }
    ],
    "question": {
      "ru": "Почему отказ от обвинения в одном тексте требует различить своё и чужое, а в другом — преобразовать отношение к бывшему?",
      "en": "Why does leaving accusation behind require distinguishing one’s own from another’s in one text, and transforming the relation to what has been in the other?"
    }
  },
  "demo:shared-world": {
    "argument": {
      "ru": "Эти места задают разные основания совместности: принадлежность целому и определённый способ одаривать и любить. Сравнение спрашивает, что каждое основание делает существенным в отношении к другому, сохраняя различие исходных мер.",
      "en": "These passages give different grounds for life with others: belonging to a whole and a particular way of bestowing and loving. Comparison asks what each ground makes significant in a relation to another while preserving the difference in their starting measures.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":750,"e":914},
        {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-09b9bca89edae4834da231d217a4db61"],"s":0},
        {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c","tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c"],"s":29,"e":124}
      ]
    },
    "grounds": [
      {
        "ref": "marcus6-44",
        "cite": [
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":750,"e":971}
        ]
      },
      {
        "ref": "z-pitiful",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-bb657a18dea91c343e6156d5111bc7e4","tos.text-unit.sid-bb657a18dea91c343e6156d5111bc7e4"]},
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-e265be83aa60fd32629dc350aa3c9ebc","tos.text-unit.sid-e265be83aa60fd32629dc350aa3c9ebc"]},
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-09b9bca89edae4834da231d217a4db61","tos.text-unit.sid-09b9bca89edae4834da231d217a4db61"]},
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c","tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c"],"s":29,"e":124}
        ]
      }
    ],
    "question": {
      "ru": "Как различаются довод от общего города и довод от создающей любви, когда речь идёт о помощи?",
      "en": "How do an argument from the shared city and an argument from creating love differ when help is at issue?"
    }
  },
  "demo:heraclitus": {
    "argument": {
      "ru": "Из фрагмента о реке можно поставить вопрос, чем сохраняется единство меняющегося, но нельзя сразу получить положение о возвращении всех состояний. Сопоставление показывает, что постоянство порядка и перемена составляющего могут мыслиться совместно, ещё до вопроса о полном повторе.",
      "en": "The river fragment raises the question of what preserves the unity of something changing, but does not immediately yield a claim that all states return. Comparison shows that a continuing order and changing constituents can be thought together before the question of complete recurrence arises.",
      "cite": [{"w":"heraclitus-book","loc":"B12"},{"w":"heraclitus-book","loc":"B30"}]
    },
    "grounds": [
      {
        "ref": "heraclitus-river",
        "focus": {
          "ru": "Окружающее рассуждение об испарении и душе принадлежит передающему свидетельству.",
          "en": "The surrounding discussion of exhalation and soul belongs to the transmitting witness."
        },
        "cite": [{"w":"heraclitus-book","loc":"B12"}]
      },
      {"ref":"heraclitus-cosmos","cite":[{"w":"heraclitus-book","loc":"B30"}]}
    ],
    "question": {
      "ru": "Что в каждом фрагменте сохраняет единство: прежний состав, имя целого или мера его изменения?",
      "en": "What preserves unity in each fragment: its former constituents, the whole’s name or the measure of its change?"
    }
  },
  "demo:stoics": {
    "argument": {
      "ru": "Первый раздел Эпиктета и VI.44 Марка Аврелия отвечают на разные вопросы о действии. Вместе тексты дают две точки входа: границу собственного и основание того, ради чего оно действует.",
      "en": "Epictetus’s first section and Marcus Aurelius VI.44 address different questions about action. Together the texts provide two entrances: the boundary of one’s own and the ground of what it acts for.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":0,"e":328},
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":2,"e":749},
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":810,"e":914}
      ]
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      },
      {
        "ref": "marcus6-44",
        "cite": [
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
        ]
      }
    ],
    "question": {
      "ru": "На какой вопрос отвечает граница собственного у Эпиктета и какой следующий вопрос открывает общественная природа у Марка Аврелия?",
      "en": "Which question does Epictetus’s boundary of one’s own answer, and which further question does Marcus Aurelius’s social nature open?"
    }
  },
  "demo:providence": {
    "argument": {
      "ru": "Ход фрагмента показывает не только веру в провидение, но и то, каким основанием действие располагает при переходе между предположениями.",
      "en": "The fragment’s movement shows both belief in providence and the ground available to action as the suppositions change.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":2,"e":455},
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":78,"e":420},
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":421,"e":845},
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":810,"e":971}
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
      "ru": "Какое основание полезного сохраняется в последней ветви рассуждения и от каких прежних предположений оно уже не зависит?",
      "en": "Which ground of usefulness remains in the argument’s last branch, and which earlier suppositions does it no longer depend on?"
    }
  },
  "demo:conatus": {
    "argument": {
      "ru": "Поэтому conatus обозначает сам способ действия существующего, а не добавленную к нему сознательную цель.",
      "en": "Conatus therefore names the existing thing’s very way of acting, rather than a conscious aim added to it.",
      "cite": [{"w":"ethica","loc":"III.P6"},{"w":"ethica","loc":"III.P7"},{"w":"ethica","loc":"III.P8"}]
    },
    "grounds": [
      {"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]},
      {"ref":"spinoza3p7-8","cite":[{"w":"ethica","loc":"III.P7"},{"w":"ethica","loc":"III.P8"}]}
    ],
    "question": {
      "ru": "Почему переход от «вещь стремится» к «стремление есть её сущность» меняет смысл стремления?",
      "en": "Why does the move from “a thing strives” to “striving is its essence” change the meaning of striving?"
    }
  },
  "demo:leibniz": {
    "argument": {
      "ru": "Так основание мира раскрывается через связку возможности и выбора, причём существование одного мира не устраняет мыслимость остальных.",
      "en": "The world’s ground is thus unfolded through possibility and choice, while one world’s existence does not remove the thinkability of the others.",
      "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
    },
    "grounds": [
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
      }
    ],
    "question": {
      "ru": "Зачем §55 различает познание лучшего, его выбор и осуществление, если речь идёт об одном существующем мире?",
      "en": "Why does §55 distinguish knowing the best, choosing it and bringing it about when one existing world is at issue?"
    }
  },
  "demo:possible-worlds": {
    "argument": {
      "ru": "Поэтому другой возможный мир и ещё одно возвращение этой жизни различаются уже по постановке вопроса. Первый вводит альтернативу существующему; второе сохраняет существовавший ряд и спрашивает об отношении к его повтору.",
      "en": "Another possible world and another recurrence of this life therefore differ from the formulation of the question onward. The first introduces an alternative to what exists; the second preserves the former sequence and asks about one’s relation to its repetition.",
      "cite": [
        {"w":"monadologie","loc":"53"},
        {"w":"monadologie","loc":"54"},
        {"w":"monadologie","loc":"55"},
        {"w":"die-froehliche-wissenschaft","loc":"341"}
      ]
    },
    "grounds": [
      {"ref":"leibniz53-55","cite":[{"w":"monadologie","loc":"53"}]},
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ],
    "question": {
      "ru": "Что меняется в вопросе о возвращении, если разрешить выбрать другую возможную жизнь?",
      "en": "What changes in the question of recurrence if choosing another possible life is allowed?"
    }
  },
  "demo:kierkegaard": {
    "argument": {
      "ru": "Понятие повторения возникает между проверкой через действие и формулой движения вперёд и назад. Поскольку здесь говорит исследующий повествователь, начало следует читать как постановку опыта: оно ещё не сообщает его результата и не снимает трудности собственного определения.",
      "en": "The concept of repetition emerges between a test through action and a formula of movement forwards and backwards. Since an investigating narrator speaks here, the opening should be read as setting up an inquiry: it has not yet reported its result or removed the difficulty of its own definition.",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    },
    "grounds": [{"ref":"kierkegaard-repetition","cite":[{"w":"gjentagelsen","loc":"opening"}]}],
    "question": {
      "ru": "Что добавляет замысел поездки к определению повторения, данному в том же абзаце?",
      "en": "What does the proposed journey add to the definition of repetition given in the same paragraph?"
    }
  },
  "demo:repetition": {
    "argument": {
      "ru": "Образ попытки ускользнуть из жизни в самый миг рождения, сославшись на забытое, возвращает формулу направлений к вопросу о том, останется ли движение внутри жизни.",
      "en": "The image of trying to slip out of life at the very moment of birth, pleading something forgotten, returns the formula of directions to the question of whether movement will remain within life.",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    },
    "grounds": [{"ref":"kierkegaard-repetition","cite":[{"w":"gjentagelsen","loc":"opening"}]}],
    "question": {
      "ru": "Почему «вспоминать вперёд» труднее понять, чем просто снова оказаться в прежнем месте?",
      "en": "Why is “recollecting forwards” harder to understand than simply being in the former place again?"
    }
  },
  "demo:camus": {
    "argument": {
      "cite": [
        {"w":"le-mythe-de-sisyphe","loc":"return"},
        {"w":"le-mythe-de-sisyphe","loc":"tragic"},
        {"w":"le-mythe-de-sisyphe","loc":"descent"},
        {"w":"le-mythe-de-sisyphe","loc":"last"}
      ]
    },
    "grounds": [{"ref":"camus-sisyphus","cite":[{"w":"le-mythe-de-sisyphe","loc":"whole"}]}],
    "question": {
      "ru": "Почему Камю помещает перемену отношения к уделу именно во время спуска, когда камень уже снова внизу?",
      "en": "Why does Camus place the change in relation to fate during the descent, when the stone is already below again?"
    }
  },
  "demo:sisyphus": {
    "argument": {
      "ru": "Заключительный образ счастья связан с устойчивым усилием и сознанием своего удела, а не с воображаемым последним подъёмом, после которого задача исчезнет.",
      "en": "The concluding image of happiness is connected with continuing effort and awareness of one’s lot, rather than an imagined final ascent after which the task disappears.",
      "cite": [
        {"w":"le-mythe-de-sisyphe","loc":"return"},
        {"w":"le-mythe-de-sisyphe","loc":"tragic"},
        {"w":"le-mythe-de-sisyphe","loc":"descent"},
        {"w":"le-mythe-de-sisyphe","loc":"last"}
      ]
    },
    "grounds": [{"ref":"camus-sisyphus","cite":[{"w":"le-mythe-de-sisyphe","loc":"whole"}]}],
    "question": {
      "ru": "Что должно измениться для заключительного счастья, если ни тяжесть камня, ни повтор труда не исчезают?",
      "en": "What must change for the concluding happiness if neither the stone’s weight nor recurring labour disappears?"
    }
  },
  "demo:deleuze": {
    "argument": {
      "cite": [
        {"w":"difference-et-repetition","loc":"intro.generality"},
        {"w":"difference-et-repetition","loc":"intro.movement"},
        {"w":"difference-et-repetition","loc":"intro.insurmountable"},
        {"w":"difference-et-repetition","loc":"intro.selective"},
        {"w":"difference-et-repetition","loc":"intro"}
      ]
    },
    "grounds": [{"ref":"deleuze-repetition","cite":[{"w":"difference-et-repetition","loc":"intro"}]}],
    "question": {
      "ru": "Почему для Делёза изображение движения ещё не совершает того движения, которое он ищет в повторении?",
      "en": "Why, for Deleuze, does representing movement not yet perform the movement he seeks in repetition?"
    }
  },
  "demo:return-difference": {
    "argument": {
      "ru": "Прочтение Делёза — определённая интерпретация ницшеанских сцен: она связывает упрёк с содержанием ответа, затем раскрывает возвращение как созидание и разрушение. Дальнейшая речь зверей о той же жизни создаёт точную задачу для сравнения: нужно объяснить, как её содержание связано с упрёком шарманке и с продолжающимся выздоровлением Заратустры.",
      "en": "Deleuze’s reading is a definite interpretation of the Nietzschean scenes: it connects the rebuke with the answer’s content and then unfolds recurrence as creation and destruction. The animals’ later speech about the same life creates a precise task for comparison: explaining how its content relates to the barrel-organ rebuke and Zarathustra’s continuing convalescence.",
      "cite": [
        {"w":"difference-et-repetition","loc":"intro.corrections"},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"],"s":1,"e":149},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
        {"w":"difference-et-repetition","loc":"intro.selective"}
      ]
    },
    "grounds": [
      {
        "ref": "deleuze-repetition",
        "cite": [
          {"w":"difference-et-repetition","loc":"intro.corrections"},
          {"w":"difference-et-repetition","loc":"intro.selective"}
        ]
      },
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"],"s":0}
        ]
      }
    ],
    "question": {
      "ru": "Какие подробности сцен позволяют решить, исправляет ли Заратустра содержание ответа, способ произнесения или оба сразу?",
      "en": "Which details of the scenes help determine whether Zarathustra corrects the answer’s content, its manner of utterance or both?"
    }
  }
};
