// Reading prompts of the local demonstration's reader. A move names its place
// in the original (`paragraphs` by the original language of the supplied catalog,
// `cite` in the bonsai witnesses for Zarathustra); no translation gets
// positions chosen by hand, a translation's place follows from the alignment.
/*
 * Own words are kept only for what a quotation cannot give (a reading, a
 * reconstructed argument, a distinction, a connection, a question, a method,
 * context). What a work itself says is never retold: a form cites the place
 * instead ({ru, en, cite}), and the stand shows the work's own text there —
 * the original and, through the alignment of the witnesses, the reader's
 * language (bonsai/zarathustra/witnesses.json, fragments.json). */
export const GUIDE_CATALOG_SHA256 = "6dc0e18c0cc4faa6630872e01a283877eecf4f2d5fb012e637e2ed6317613ba0";
export const CLOSE_READING_GUIDES = {
  "z-vision": {
    "orientation": {
      "ru": "Проверяйте, что меняется между этими способами говорить о возвращении.",
      "en": "Examine what changes between these ways of speaking about recurrence.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86","tos.text-unit.sid-27d8a9bb60151a6a7946b0e09a848f75"],"s":229},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"],"s":73},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"],"s":39},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"],"s":53}
      ]
    },
    "moves": [
      {
        "title": {"ru":"Уверенность и возражение","en":"Confidence and objection"},
        "body": {
          "ru": "Сопоставьте ответ карлика о круге с немедленным возражением Заратустры. Возражение может касаться лёгкости ответа, его содержания или обоих сразу. Какая следующая реплика помогает различить эти возможности?",
          "en": "Compare the dwarf’s answer about the circle with Zarathustra’s immediate objection. The objection may concern the answer’s ease, its content or both. Which subsequent remark helps distinguish these possibilities?"
        },
        "paragraphs": {"ru":30,"en":30,"de":30}
      },
      {
        "title": {"ru":"От довода к видению","en":"From argument to vision"},
        "body": {
          "ru": "Проследите этот разрыв: воспоминание не служит само по себе доказательством кругового времени, но меняет способ присутствия прошлого в сцене.",
          "en": "Follow this break: recollection is not by itself proof of circular time, but changes how the past is present in the scene.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa"],"s":106}
          ]
        },
        "paragraphs": {"ru":39,"en":39,"de":39}
      },
      {
        "title": {"ru":"Кто совершает освобождение","en":"Who performs the release"},
        "body": {
          "ru": "Различите помощь, собственное действие и уже достигнутое преображение: эти роли не совпадают.",
          "en": "Distinguish assistance, one’s own action and an accomplished transformation: these roles do not coincide.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e"],"e":97},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
          ]
        },
        "paragraphs": {"ru":47,"en":48,"de":48}
      }
    ],
    "question": {
      "ru": "Какое чтение сохраняет одновременно вопросы о времени, безуспешную попытку вытянуть змею и ещё не утолённую жажду смеха?",
      "en": "Which reading preserves the questions about time, the failed attempt to pull out the serpent and the still unfulfilled longing for laughter together?"
    },
    "limit": {
      "ru": "Текст не снабжает каждый образ единственным готовым соответствием.",
      "en": "The text does not assign each image a single ready meaning.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-47ecea77ae8a30862d06ed7c80ebdf1a","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"]}
      ]
    }
  },
  "z-convalescent": {
    "orientation": {
      "ru": "Сравнивайте способность произнести учение со способностью вынести то, что в нём возвращается.",
      "en": "Compare the ability to state a teaching with the ability to bear what returns in it.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-711dc06c5cfe15d63a5af8a9195a5638"]}
      ]
    },
    "moves": [
      {
        "title": {"ru":"Песня и цена понимания","en":"Song and the cost of understanding"},
        "body": {
          "ru": "Определите, что песня передаёт, а что сглаживает: различие может лежать в пережитом усилии, даже если слова о возвращении сходны.",
          "en": "Identify what the song conveys and what it smooths over: the difference may concern lived effort even where the words about recurrence are similar.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-c1a61ec1014026941c54254a0978115d"]}
          ]
        },
        "paragraphs": {"ru":24,"en":24,"de":24}
      },
      {
        "title": {"ru":"Возвращается и отвергаемое","en":"What one rejects also returns"},
        "body": {
          "ru": "Проследите, почему малость человека становится возражением против существования в целом. Отвращение здесь нельзя устранить, оставив в возвращении только великое или желанное: тогда изменится само условие испытания.",
          "en": "Trace why human smallness becomes an objection to existence as a whole. Disgust cannot be removed by retaining only greatness or what is wanted in recurrence: that would change the condition of the trial itself."
        },
        "paragraphs": {"ru":43,"en":43,"de":43}
      },
      {
        "title": {"ru":"Кому принадлежит последняя формула","en":"Whose formula comes last"},
        "body": {
          "ru": "Сопоставьте молчание Заратустры с разговором с собственной душой, сохранив различие между воображаемым высказыванием и ответом.",
          "en": "Compare Zarathustra’s silence with his conversation with his soul, preserving the difference between an imagined utterance and a response.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"],"e":169}
          ]
        },
        "paragraphs": {"ru":63,"en":63,"de":63}
      }
    ],
    "question": {
      "ru": "Чем отличалось бы усвоенное учение от верно воспроизведённой формулы в этой сцене?",
      "en": "What would distinguish an assimilated teaching from a correctly repeated formula in this scene?"
    },
    "limit": {
      "ru": "Молчание ограничивает уверенность атрибуции, но само по себе не доказывает ни согласия, ни отказа.",
      "en": "Silence limits confidence in attribution, but by itself proves neither assent nor rejection."
    }
  },
  "z-redemption": {
    "orientation": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-04ae7c3916e9390af927106df7b960ca","tos.text-unit.sid-f43f59a737f30ba85dd451b8e820cbce"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be"],"s":72},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-77d2134e63b47a686837068668c73b22","tos.text-unit.sid-225401ec25fcfee3ede1e91e2605ec08"],"s":25}
      ]
    },
    "moves": [
      {
        "title": {"ru":"Граница и ответ на неё","en":"A limit and the response to it"},
        "body": {
          "ru": "Выделите промежуточный шаг. Почему необратимость времени сама по себе ещё не требует мести?",
          "en": "Isolate the intervening step. Why does time’s irreversibility not itself require revenge?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-bc8111269b7adf1e6f1a2c297ae8b00b"],"s":72}
          ]
        },
        "paragraphs": {"ru":21,"en":21,"de":21}
      },
      {
        "title": {"ru":"Неоконченный ответ","en":"An unfinished answer"},
        "body": {
          "ru": "Проверьте, что действительно решено, а что пока только обещано.",
          "en": "Examine what has actually been resolved and what has only been promised.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b","tos.text-unit.sid-ff240a868709abef0da385e34299d2c3"]},
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-339e330ca6a6a19079d4f5a3194cb1f7","tos.text-unit.sid-b074f36127c044db8c943822e45c643a"]}
          ]
        },
        "paragraphs": {"ru":39,"en":39,"de":39}
      },
      {
        "title": {"ru":"Слушатель спрашивает о говорящем","en":"The listener questions the speaker"},
        "body": {
          "ru": "Перечитайте завершающий обмен: меняется ли только способ объяснения или то, что Заратустра способен сказать каждому из слушателей?",
          "en": "Reread the closing exchange: does only the manner of explanation change, or also what Zarathustra can say to each listener?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-77d2134e63b47a686837068668c73b22","tos.text-unit.sid-225401ec25fcfee3ede1e91e2605ec08"],"s":25}
          ]
        },
        "paragraphs": {"ru":48,"en":48,"de":48}
      }
    ],
    "question": {
      "ru": "Почему после слов о созидающей воле Заратустра снова спрашивает, кто научил волю хотеть назад, и прерывает свою речь?",
      "en": "Why, after speaking of the creating will, does Zarathustra again ask who taught the will to will backwards and break off his speech?"
    },
    "limit": {
      "ru": "Прерывание речи и вопрос горбуна оставляют различие между объявленным избавлением и состоявшимся ответом частью самой сцены.",
      "en": "The interrupted speech and the hunchback’s question keep the difference between announced redemption and an accomplished answer inside the scene itself."
    }
  },
  "z-prologue": {
    "orientation": {
      "ru": "Первый раздел предисловия ещё не показывает, как люди примут дар. Внимание к этому разрыву удерживает разницу между намерением говорящего и состоявшейся встречей.",
      "en": "The prologue’s first section does not yet show how people receive the gift. Attending to this gap preserves the difference between the speaker’s intention and an accomplished encounter.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"],"s":208}
      ]
    },
    "moves": [
      {
        "title": {"ru":"Свет нуждается в принимающем","en":"Light needs a receiver"},
        "body": {
          "ru": "Проследите, как речь переходит от получения солнечного избытка к желанию отдать собственную мудрость.",
          "en": "Trace how the address moves from receiving the sun’s abundance to wanting to give away his own wisdom.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-87ee03096d4da574543c6c6cdeab9b26","tos.text-unit.sid-87ee03096d4da574543c6c6cdeab9b26"],"s":21},
            {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb","tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb"],"s":7}
          ]
        },
        "paragraphs": {"ru":1,"en":1,"de":1}
      },
      {
        "title": {"ru":"Переполнение и опустошение","en":"Overflowing and emptying"},
        "body": {
          "ru": "Сопоставьте этот конец с началом раздела. Что связывает опустошение с переменой сердца?",
          "en": "Compare this ending with the section’s opening. What connects emptying with the change of heart?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-778b42e35fce3f7979a96c19045ceb9c","tos.text-unit.sid-2784e2f54e2c96f665b5985a1285d127"]},
            {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46"],"s":116}
          ]
        },
        "paragraphs": {"ru":9,"en":9,"de":9}
      }
    ],
    "question": {
      "ru": "Почему желание отдать мудрость выражено одновременно через переливающуюся чашу, нисхождение и желание снова стать человеком?",
      "en": "Why is the desire to give wisdom expressed through an overflowing cup, a descent and a wish to become human again?"
    },
    "limit": {
      "ru": "Реакция слушателей в последующих разделах здесь не представлена.",
      "en": "The listeners’ responses in later sections are not supplied here.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-2784e2f54e2c96f665b5985a1285d127"]}
      ]
    }
  },
  "z-metamorphoses": {
    "orientation": {
      "ru": "Читайте переходы через предел предыдущей способности: сила, необходимая на одном шаге, ещё не выполняет работу следующего.",
      "en": "Read each transition through the previous capacity’s limit: the strength required at one step does not yet perform the next step’s work.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-828cde9bb01454dcf343b639d4bc69a6","tos.text-unit.sid-4eb9cdd31d7dd3d70e40c2b087e579c7"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-9089a1d79a3475324bf27fde5b1eb037","tos.text-unit.sid-ac6059bf23968ca1320ca593f57ccf72"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
      ]
    },
    "moves": [
      {
        "title": {"ru":"Почему недостаточно льва","en":"Why the lion is insufficient"},
        "body": {
          "ru": "Разведите устранение чужого приказа и появление собственного основания действовать.",
          "en": "Separate removal of another’s command from the emergence of one’s own grounds for acting.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]}
          ]
        },
        "paragraphs": {"ru":18,"en":18,"de":18}
      },
      {
        "title": {"ru":"Что начинает ребёнок","en":"What the child begins"},
        "body": {
          "ru": "Найдите переход от священного «нет» льва к священному «да» ребёнка. Почему для созидания оказывается недостаточно завоевать право говорить «я хочу»?",
          "en": "Find the transition from the lion’s sacred No to the child’s sacred Yes. Why is winning the right to say “I will” still insufficient for creation?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"]}
          ]
        },
        "paragraphs": {"ru":22,"en":22,"de":22}
      }
    ],
    "question": {
      "ru": "Как отличить новое основание жизни от реакции, всё ещё зависящей от отвергнутого приказа?",
      "en": "How can a new ground for life be distinguished from a reaction still dependent on the rejected command?"
    },
    "limit": {
      "ru": "Последовательность образов не задаёт календарной модели взросления и не доказывает, что человек раз и навсегда покидает прежние способности.",
      "en": "The sequence of images supplies no chronological model of maturation and does not prove that a person leaves earlier capacities behind once and for all."
    }
  },
  "epictetus-control": {
    "orientation": {
      "ru": "Проверьте, как классификация должна изменить суждение, не подменяя описание внешнего события.",
      "en": "Examine how classification is meant to change judgment without replacing description of the external event.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"e":52},
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":1067,"e":1160}
      ]
    },
    "moves": [
      {
        "title": {"ru":"Власть над чем именно","en":"Power over what exactly"},
        "body": {
          "ru": "Какой смысл «вреда» позволяет связать столь сильное обещание с исходным перечислением?",
          "en": "What sense of “harm” connects such a strong promise with the opening list?",
          "cite": [
            {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":212},
            {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":329,"e":721}
          ]
        },
        "paragraphs": {"ru":0,"en":0,"grc":0}
      },
      {
        "title": {"ru":"Проверка впечатления","en":"Examining an impression"},
        "body": {
          "ru": "Сопоставьте то, от чего предлагается отказаться, с правилом различения своего и внешнего. Почему эту работу нельзя выполнить одним признанием формулы?",
          "en": "Compare what is to be relinquished with the rule distinguishing one’s own from the external. Why can this work not be accomplished merely by accepting a formula?",
          "cite": [
            {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":867,"e":1066}
          ]
        },
        "paragraphs": {"ru":1,"en":1,"grc":3}
      }
    ],
    "question": {
      "ru": "Что меняется в суждении о впечатлении, когда предмет, который казался своим, признан внешним?",
      "en": "What changes in judging an impression when something that seemed one’s own is recognised as external?"
    },
    "limit": {
      "ru": "Отдельный вопрос об обязанностях перед людьми требует других мест «Энхиридиона»; его нельзя решать, смягчив заключительное «это меня не касается».",
      "en": "Duties towards others require other passages of the Enchiridion; that question cannot be settled by softening the final “it does not concern you”.",
      "cite": [
        {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
      ]
    }
  },
  "marcus-fate": {
    "orientation": {
      "ru": "Проследите, что сохраняется при смене этих предпосылок.",
      "en": "Trace what survives the change of premises.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":2,"e":186},
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":260,"e":749}
      ]
    },
    "moves": [
      {
        "title": {"ru":"От промысла к собственной природе","en":"From providence to one’s own nature"},
        "body": {
          "ru": "Поэтому отсутствие уверенности в частном промысле не заканчивается отказом действовать. Но откуда берётся нормативная сила разумной и общественной природы?",
          "en": "Lack of certainty about individual providence therefore does not end in refusal to act. But where does the normative force of rational and social nature come from?",
          "cite": [
            {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"],"s":421,"e":914}
          ]
        },
        "paragraphs": {"ru":0,"en":0,"grc":1}
      }
    ],
    "question": {
      "ru": "Как довод от разумной и общественной природы приводит к заключению, что полезное городам полезно мне?",
      "en": "How does the argument from rational and social nature lead to the conclusion that what benefits the cities benefits me?"
    },
    "limit": {
      "ru": "Более широкое учение о полезном потребовало бы чтения других мест.",
      "en": "A broader account of benefit would require reading other passages.",
      "cite": [
        {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
      ]
    }
  },
  "spinoza-freedom": {
    "orientation": {
      "ru": "Сначала удержите обе части, прежде чем переносить сюда привычную противоположность свободы и необходимости.",
      "en": "First retain both parts before importing the familiar opposition between freedom and necessity.",
      "cite": [{"w":"ethica","loc":"I.D7"}]
    },
    "moves": [
      {
        "title": {"ru":"Источник определения действия","en":"The source of an action’s determination"},
        "body": {
          "ru": "Поэтому случайность поступка ещё не делает его свободным. Что потребовалось бы установить о природе действующего, чтобы применить определение?",
          "en": "An action’s randomness therefore does not yet make it free. What would have to be established about the agent’s nature to apply the definition?",
          "cite": [{"w":"ethica","loc":"I.D7"}]
        },
        "paragraphs": {"ru":0,"en":0,"la":0}
      }
    ],
    "question": {
      "ru": "Можем ли мы назвать человеческий поступок свободным на основании одного этого определения, не рассмотрев место человека в системе причин?",
      "en": "Can a human action be called free from this definition alone, without examining the human being’s place in the system of causes?"
    },
    "limit": {
      "ru": "Перед нами определение I.7, а не готовая психология человеческой свободы. Оно само не устанавливает, какой конкретный человек удовлетворяет его условиям.",
      "en": "This is definition I.7, not a ready psychology of human freedom. It does not by itself establish which particular human being meets its conditions."
    }
  },
  "spinoza-conatus": {
    "orientation": {
      "ru": "Читайте стремление шире сознательного желания и проверьте, почему это различие требуется самим доводом.",
      "en": "Read striving more broadly than conscious desire and examine why the argument itself requires this distinction.",
      "cite": [{"w":"ethica","loc":"III.P6"}]
    },
    "moves": [
      {
        "title": {"ru":"Оговорка внутри тезиса","en":"The qualification inside the proposition"},
        "body": {
          "ru": "Внешнее разрушение этим не исключается. Различите стремление продолжаться и гарантию сохранения: иначе каждый погибший организм стал бы слишком быстрым опровержением.",
          "en": "External destruction is not excluded. Distinguish striving to continue from a guarantee of preservation; otherwise every organism that perishes would become too quick a refutation.",
          "cite": [{"w":"ethica","loc":"III.P6"}]
        },
        "paragraphs": {"ru":0,"en":0,"la":0}
      },
      {
        "title": {"ru":"Что несёт доказательство","en":"What bears the proof"},
        "body": {
          "ru": "Укажите, какие из этих посылок требуется принять. Повседневное наблюдение упорства не заменяет их обоснования.",
          "en": "Identify which premises must be accepted. Everyday observation of persistence does not replace their justification.",
          "cite": [{"w":"ethica","loc":"III.P6"}]
        },
        "paragraphs": {"ru":1,"en":1,"la":1}
      }
    ],
    "question": {
      "ru": "Почему доказательство относится ко всем отдельным вещам, хотя слово «стремится» может напомнить читателю о сознательном желании?",
      "en": "Why does the proof concern all individual things, although “endeavours” may remind a reader of conscious desire?"
    },
    "limit": {
      "ru": "III.6 с доказательством ещё не отождествляет conatus с осознанным желанием. Различения следующих теорем нельзя подменить привычным значением слова «стремление».",
      "en": "III.6 and its proof do not yet identify conatus with conscious desire. The everyday sense of “striving” cannot replace the distinctions of the subsequent propositions.",
      "cite": [{"w":"ethica","loc":"III.P9.S"}]
    }
  },
  "leibniz-worlds": {
    "orientation": {
      "ru": "Довод оценивает миры как целые, поэтому нельзя незаметно заменить предмет сравнения отдельным приятным событием.",
      "en": "The argument evaluates worlds as wholes, so a single agreeable event cannot silently replace its object of comparison.",
      "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
    },
    "moves": [
      {
        "title": {"ru":"От множества возможностей к выбору","en":"From many possibilities to choice"},
        "body": {
          "ru": "Выделите добавленную посылку: требование основания само по себе ещё не говорит, каким именно должно быть основание.",
          "en": "Isolate the added premise: requiring a reason does not by itself say which reason it must be.",
          "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"}]
        },
        "paragraphs": {"ru":0,"en":0,"fr":0}
      },
      {
        "title": {"ru":"Знание, благо и осуществление","en":"Knowledge, goodness and realization"},
        "body": {
          "ru": "Если мысленно убрать одну способность, следует ли ещё существование наилучшего? Этот разбор показывает, сколько предпосылок скрывает краткая формула.",
          "en": "If one capacity is removed in thought, does the existence of the best still follow? This analysis reveals how many premises the short formula contains.",
          "cite": [{"w":"monadologie","loc":"55"}]
        },
        "paragraphs": {"ru":2,"en":2,"fr":2}
      }
    ],
    "question": {
      "ru": "Какие основания позволили бы судить о целом мире из нашей ограниченной перспективы на его события?",
      "en": "What grounds would permit judging a whole world from our limited perspective on its events?"
    },
    "limit": {
      "ru": "§§53–55 не дают человеку перечня всех возможных миров или процедуры подсчёта их совершенства.",
      "en": "§§53–55 give a human reader neither a list of all possible worlds nor a procedure for calculating their perfection."
    }
  },
  "kierkegaard-repetition": {
    "orientation": {
      "ru": "Сохраните условность вопроса о возможности повторения, прежде чем счесть формулу итогом книги.",
      "en": "Preserve the conditional question of repetition’s possibility before treating the formula as the book’s conclusion.",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    },
    "moves": [
      {
        "title": {"ru":"Обратное и поступательное движение","en":"Backward and forward movement"},
        "body": {
          "ru": "Какие признаки позволят рассказчику отличить неудачу повторения от изменения собственного ожидания?",
          "en": "Which signs would let the narrator distinguish repetition’s failure from a change in his own expectation?",
          "cite": [{"w":"gjentagelsen","loc":"opening"}]
        },
        "paragraphs": {"ru":0,"en":0,"da":0}
      },
      {
        "title": {"ru":"Пример и понятие","en":"Example and concept"},
        "body": {
          "ru": "Почему рассказ начинается с такого примера — и что поездка должна показать сверх возможности вновь оказаться в том же городе?",
          "en": "Why does the account begin with this example, and what must the journey show beyond the possibility of being in the same city again?",
          "cite": [{"w":"gjentagelsen","loc":"opening"}]
        },
        "paragraphs": {"ru":0,"en":0,"da":0}
      }
    ],
    "question": {
      "ru": "Если знакомое место не возвращает прежнюю радость, что именно не повторилось: место, опыт или отношение к нему?",
      "en": "If a familiar place does not restore an earlier joy, what failed to repeat: the place, the experience or one’s relation to it?"
    },
    "limit": {
      "ru": "Начальный абзац ещё не содержит результатов берлинского опыта или последующего движения книги.",
      "en": "The opening paragraph contains neither the results of the Berlin experiment nor the book’s later movement.",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    }
  },
  "heraclitus-river": {
    "orientation": {
      "ru": "Фрагмент дошёл внутри чужого свидетельства, рядом с рассуждением о душе. Прежде чем превратить реку в отдельный афоризм, различите переданные слова и объясняющую рамку передающего.",
      "en": "The fragment survives within another writer’s testimony, alongside an account of the soul. Before turning the river into an isolated aphorism, distinguish the reported words from the transmitter’s explanatory frame."
    },
    "moves": [
      {
        "title": {"ru":"Та же река, другие воды","en":"The same river, different waters"},
        "body": {
          "ru": "Попробуйте определить тождество через материал, русло и порядок движения: каждый критерий даст другой ответ. Затем вернитесь к окружающим словам о душе и спросите, какую работу сравнение выполняет там.",
          "en": "Try defining identity through material, channel and pattern of movement: each criterion gives a different answer. Then return to the surrounding words about the soul and ask what work the comparison performs there.",
          "cite": [{"w":"heraclitus-book","loc":"B12"}]
        },
        "paragraphs": {"ru":0,"en":0,"grc":0}
      }
    ],
    "question": {
      "ru": "Какой вид устойчивости позволяет распознать изменение, не делая изменяющееся неподвижным?",
      "en": "Which kind of stability allows change to be recognized without making the changing thing motionless?"
    },
    "limit": {
      "ru": "Это конкретное свидетельство о реке; оно не тождественно всем поздним вариантам изречения о невозможности войти дважды.",
      "en": "This is a particular river testimony; it is not identical to every later version of the saying about being unable to enter twice."
    }
  },
  "heraclitus-cosmos": {
    "orientation": {
      "ru": "Смысл возникает в совместном удержании изменения и порядка, а не в выборе только одной стороны.",
      "en": "Meaning emerges by retaining change and order together, rather than selecting only one side.",
      "cite": [{"w":"heraclitus-book","loc":"B30"}]
    },
    "moves": [
      {
        "title": {"ru":"Что сохраняет мера","en":"What measure preserves"},
        "body": {
          "ru": "Сопоставьте вечную жизнь огня с мерой его возгорания и угасания. Из постоянства такого процесса ещё не следует буквальный возврат каждой детали мира. Какую дополнительную посылку пришлось бы внести, чтобы перейти от меры к одинаковой последовательности событий?",
          "en": "Compare the fire’s everlasting life with the measures of its kindling and extinction. Persistence of such a process does not yet imply the literal return of every worldly detail. Which extra premise would be needed to move from measure to an identical sequence of events?"
        },
        "paragraphs": {"ru":0,"en":0,"grc":0}
      }
    ],
    "question": {
      "ru": "Как описать порядок, который существует через превращения, а не за их пределами?",
      "en": "How can an order be described that exists through transformations rather than outside them?"
    },
    "limit": {
      "ru": "Сопоставление с вечным возвращением — вопрос стенда; данный фрагмент сам не устанавливает тождество двух учений.",
      "en": "Comparison with eternal recurrence is this stand’s question; the fragment itself establishes no identity between the two teachings."
    }
  }
};

/** A changed catalog gets no old positional guidance. Reading itself stays available. */
export function bindCloseReadingGuides(catalog,sha256){
 if(sha256!==GUIDE_CATALOG_SHA256)return new Map();
 const documents=new Map(catalog.passages.map(item=>[item.id,item])),bound=new Map();
 for(const [id,guide]of Object.entries(CLOSE_READING_GUIDES)){
  const doc=documents.get(id);if(doc?.status!=='available')throw Error('Reading guide requires its available text: '+id);
  for(const step of guide.moves)for(const [code,index]of Object.entries(step.paragraphs??{}))if(!Number.isSafeInteger(index)||index<0||index>=doc.versions?.[code]?.paragraphs.length||!doc.versions?.[code])throw Error('Reading guide paragraph is outside its version: '+id+' '+code);
  bound.set(id,guide);
 }
 return bound;
}
