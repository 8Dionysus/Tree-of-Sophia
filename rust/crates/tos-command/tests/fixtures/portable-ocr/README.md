# Synthetic portable OCR signature fixture

These public bytes exercise receipt authentication, source/output binding and
metadata-only replay. They do not represent a real OCR execution or source
assessment. The signature was generated over the exact receipt bytes with a
fresh ephemeral P-256 test key; only its public key is retained here. No private
key, producer checkout, signing service or external verifier is needed by tests.
