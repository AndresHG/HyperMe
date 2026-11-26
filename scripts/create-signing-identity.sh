#!/bin/sh
# Creates "HyperMe Local Signing", a self-signed code-signing identity in your login keychain,
# once. scripts/bundle-macos.sh signs HyperMe.app with it, so macOS keeps Accessibility, Input
# Monitoring and Microphone grants across rebuilds (with an ad-hoc signature every rebuild looks
# like a new app and the grants are lost).
#
# The key and certificate stay in your keychain; nothing is written to the repository. macOS
# asks for your login password once, to trust the certificate for code signing.
#
# If Accessibility ever turns red after a rebuild, macOS stored that grant for one exact build
# (it did on 26 November 2026, before the identity was in use): remove HyperMe from System
# Settings > Privacy & Security > Accessibility with -, then allow it again from HyperMe. The
# new grant names this certificate and survives rebuilds.
set -eu

name="HyperMe Local Signing"
keychain="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -q "\"$name\""; then
  echo "\"$name\" already exists and is trusted for code signing."
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cat > "$work/req.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $name
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF

# macOS's own LibreSSL: its PKCS#12 output is what `security import` reads.
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$work/key.pem" -out "$work/cert.pem" -config "$work/req.cnf" 2>/dev/null
# A throwaway password for the transport file only; the file is deleted on exit.
transport=$(/usr/bin/openssl rand -hex 16)
/usr/bin/openssl pkcs12 -export -inkey "$work/key.pem" -in "$work/cert.pem" \
  -out "$work/identity.p12" -passout "pass:$transport"
security import "$work/identity.p12" -k "$keychain" -P "$transport" -T /usr/bin/codesign
echo "Trusting the certificate for code signing: macOS will ask for your login password."
security add-trusted-cert -r trustRoot -p codeSign -k "$keychain" "$work/cert.pem"

if security find-identity -v -p codesigning | grep -q "\"$name\""; then
  echo "Done: \"$name\" is ready. Now run scripts/bundle-macos.sh."
else
  echo "The identity was imported but is not trusted for code signing yet." >&2
  exit 1
fi
