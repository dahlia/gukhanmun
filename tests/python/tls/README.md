Local TLS test fixture
======================

*localhost.crt* is a self-signed certificate for `127.0.0.1`. Its matching
*localhost.key* is a public test fixture, never a production credential.
Tests trust this certificate explicitly without disabling verification.
