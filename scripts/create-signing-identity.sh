#!/bin/sh
# Creates "HyperMe Local Signing", a self-signed code-signing identity in your login keychain,
# once. scripts/bundle-macos.sh signs HyperMe.app with it, so macOS keeps Accessibility, Input
# Monitoring and Microphone grants across rebuilds (with an ad-hoc signature every rebuild looks
# like a new app and the grants are lost).
#
# The key and certificate stay in your keychain; nothing is written to the repository.
#
# The certificate is trusted for code signing twice: for you (login password) and for the whole
# Mac (administrator password, through sudo). Accessibility is decided by macOS's system privacy
# service, which runs outside your account and ignores your personal trust settings: with only
# those, it cannot check the signature, falls back to the exact build's hash, and every rebuild
# loses the Accessibility grant (seen on 26 November 2026, tccd status -67050).
# To remove it later: Keychain Access > login > My Certificates > "HyperMe Local Signing".
set -eu

name="HyperMe Local Signing"
keychain="$HOME/Library/Keychains/login.keychain-db"

system_trust() { # trust the certificate for code signing for the whole Mac
  echo "Trusting \"$name\" for code signing on this Mac: macOS asks for an administrator password."
  sudo security add-trusted-cert -d -r trustRoot -p codeSign -k /Library/Keychains/System.keychain "$1"
}

if security find-identity -v -p codesigning | grep -q "\"$name\""; then
  if security dump-trust-settings -d 2>/dev/null | grep -q "$name"; then
    echo "\"$name\" already exists and is trusted for code signing on this Mac."
    exit 0
  fi
  # Made by an earlier version of this script: trusted for you only.
  cert=$(mktemp)
  security find-certificate -c "$name" -p > "$cert"
  system_trust "$cert"
  rm -f "$cert"
  echo "Done. Rebuild with scripts/bundle-macos.sh, then switch HyperMe on in Accessibility once more."
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
system_trust "$work/cert.pem"

if security find-identity -v -p codesigning | grep -q "\"$name\""; then
  echo "Done: \"$name\" is ready. Now run scripts/bundle-macos.sh."
else
  echo "The identity was imported but is not trusted for code signing yet." >&2
  exit 1
fi
