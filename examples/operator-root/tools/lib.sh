# shellcheck shell=bash
#
# Shared helpers for the example's tool processes.
#
# The host starts each process as /bin/bash under a sandbox that allows no
# fork and no exec of anything but /bin/bash itself, and no file writes. So
# everything here is a bash 3.2 builtin: no command substitution, no pipe, no
# here-string (it writes a temporary file), and no external program. Bytes are
# kept as arrays of integers (0-255) in BYTES.
#
# What a process gets on stdin is one line of `profile-tool.request/v1`; what it
# prints on stdout is one `profile-tool.result/v1` document.

B64_ALPHABET='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

SHA256_K=(
  0x428a2f98 0x71374491 0xb5c0fbcf 0xe9b5dba5 0x3956c25b 0x59f111f1 0x923f82a4 0xab1c5ed5
  0xd807aa98 0x12835b01 0x243185be 0x550c7dc3 0x72be5d74 0x80deb1fe 0x9bdc06a7 0xc19bf174
  0xe49b69c1 0xefbe4786 0x0fc19dc6 0x240ca1cc 0x2de92c6f 0x4a7484aa 0x5cb0a9dc 0x76f988da
  0x983e5152 0xa831c66d 0xb00327c8 0xbf597fc7 0xc6e00bf3 0xd5a79147 0x06ca6351 0x14292967
  0x27b70a85 0x2e1b2138 0x4d2c6dfc 0x53380d13 0x650a7354 0x766a0abb 0x81c2c92e 0x92722c85
  0xa2bfe8a1 0xa81a664b 0xc24b8b70 0xc76c51a3 0xd192e819 0xd6990624 0xf40e3585 0x106aa070
  0x19a4c116 0x1e376c08 0x2748774c 0x34b0bcb5 0x391c0cb3 0x4ed8aa4a 0x5b9cca4f 0x682e6ff3
  0x748f82ee 0x78a5636f 0x84c87814 0x8cc70208 0x90befffa 0xa4506ceb 0xbef9a3f7 0xc67178f2
)

# request_field KEY VAR: copy the first `"KEY":"value"` of REQUEST into VAR.
# Inside a JSON string every quote is escaped, so this cannot match text the
# model chose.
request_field() {
  local pattern="\"$1\":\"([^\"]*)\""
  [[ $REQUEST =~ $pattern ]] || return 1
  printf -v "$2" '%s' "${BASH_REMATCH[1]}"
}

# read_request: read the request line and the values a result must repeat.
# The example's tools each take one input and propose one output.
read_request() {
  local pattern
  IFS= read -r REQUEST || [ -n "$REQUEST" ] || return 1
  request_field request_id REQ_ID || return 1
  request_field tool_id REQ_TOOL || return 1
  request_field runtime_sha256 REQ_RUNTIME || return 1
  request_field input_revision REQ_REVISION || return 1
  request_field schema_sha256 REQ_SCHEMA || return 1
  request_field bytes_base64 REQ_INPUT || return 1
  pattern='"outputs":\["([^"]+)"\]'
  [[ $REQUEST =~ $pattern ]] || return 1
  REQ_OUTPUT=${BASH_REMATCH[1]}
}

# b64_decode TEXT: canonical base64 into BYTES.
b64_decode() {
  local text=$1 length=${#1} i c prefix value acc=0 bits=0 n=0
  BYTES=()
  (( length % 4 == 0 )) || return 1
  for (( i = 0; i < length; i++ )); do
    c=${text:i:1}
    if [ "$c" = "=" ]; then
      (( i >= length - 2 )) || return 1
      continue
    fi
    prefix=${B64_ALPHABET%%"$c"*}
    value=${#prefix}
    (( value < 64 )) || return 1
    acc=$(( (acc << 6) | value ))
    bits=$(( bits + 6 ))
    if (( bits >= 8 )); then
      bits=$(( bits - 8 ))
      BYTES[n]=$(( (acc >> bits) & 255 ))
      n=$(( n + 1 ))
      acc=$(( acc & ((1 << bits) - 1) ))
    fi
  done
}

# b64_encode: BYTES into canonical base64 in B64.
b64_encode() {
  local n=${#BYTES[@]} i word out=""
  for (( i = 0; i < n; i += 3 )); do
    word=$(( (BYTES[i] << 16) | (${BYTES[i+1]:-0} << 8) | ${BYTES[i+2]:-0} ))
    out+=${B64_ALPHABET:$(( (word >> 18) & 63 )):1}${B64_ALPHABET:$(( (word >> 12) & 63 )):1}
    if (( i + 1 < n )); then out+=${B64_ALPHABET:$(( (word >> 6) & 63 )):1}; else out+="="; fi
    if (( i + 2 < n )); then out+=${B64_ALPHABET:$(( word & 63 )):1}; else out+="="; fi
  done
  B64=$out
}

# bytes_to_text: BYTES into TEXT. A NUL byte cannot live in a bash string.
bytes_to_text() {
  local byte hex char out=""
  for byte in "${BYTES[@]}"; do
    (( byte != 0 )) || return 1
    printf -v hex '%02x' "$byte"
    printf -v char '%b' "\\x$hex"
    out+=$char
  done
  # shellcheck disable=SC2034 # read by the tool scripts
  TEXT=$out
}

# text_to_bytes TEXT: the bytes of TEXT (LC_ALL=C) into BYTES.
text_to_bytes() {
  local text=$1 length=${#1} i code
  BYTES=()
  for (( i = 0; i < length; i++ )); do
    printf -v code '%d' "'${text:i:1}"
    BYTES[i]=$(( code & 255 ))
  done
}

# sha256_bytes: the SHA-256 of BYTES, as lowercase hex, in DIGEST.
sha256_bytes() {
  local -a m w
  local n=${#BYTES[@]} length bits i t block
  local a b c d e f g h s0 s1 ch maj t1 t2
  local h0=0x6a09e667 h1=0xbb67ae85 h2=0x3c6ef372 h3=0xa54ff53a
  local h4=0x510e527f h5=0x9b05688c h6=0x1f83d9ab h7=0x5be0cd19
  local mask=0xFFFFFFFF
  m=("${BYTES[@]}")
  m[n]=128
  length=$(( n + 1 ))
  while (( length % 64 != 56 )); do
    m[length]=0
    length=$(( length + 1 ))
  done
  bits=$(( n * 8 ))
  for (( i = 7; i >= 0; i-- )); do
    m[length]=$(( (bits >> (i * 8)) & 255 ))
    length=$(( length + 1 ))
  done
  for (( block = 0; block < length; block += 64 )); do
    for (( t = 0; t < 16; t++ )); do
      i=$(( block + t * 4 ))
      w[t]=$(( (m[i] << 24) | (m[i+1] << 16) | (m[i+2] << 8) | m[i+3] ))
    done
    for (( t = 16; t < 64; t++ )); do
      a=${w[t-15]}
      b=${w[t-2]}
      s0=$(( (((a >> 7) | (a << 25)) ^ ((a >> 18) | (a << 14)) ^ (a >> 3)) & mask ))
      s1=$(( (((b >> 17) | (b << 15)) ^ ((b >> 19) | (b << 13)) ^ (b >> 10)) & mask ))
      w[t]=$(( (w[t-16] + s0 + w[t-7] + s1) & mask ))
    done
    a=$h0 b=$h1 c=$h2 d=$h3 e=$h4 f=$h5 g=$h6 h=$h7
    for (( t = 0; t < 64; t++ )); do
      s1=$(( (((e >> 6) | (e << 26)) ^ ((e >> 11) | (e << 21)) ^ ((e >> 25) | (e << 7))) & mask ))
      ch=$(( (e & f) ^ (~e & g) ))
      t1=$(( (h + s1 + ch + SHA256_K[t] + w[t]) & mask ))
      s0=$(( (((a >> 2) | (a << 30)) ^ ((a >> 13) | (a << 19)) ^ ((a >> 22) | (a << 10))) & mask ))
      maj=$(( (a & b) ^ (a & c) ^ (b & c) ))
      t2=$(( (s0 + maj) & mask ))
      h=$g g=$f f=$e
      e=$(( (d + t1) & mask ))
      d=$c c=$b b=$a
      a=$(( (t1 + t2) & mask ))
    done
    h0=$(( (h0 + a) & mask )) h1=$(( (h1 + b) & mask ))
    h2=$(( (h2 + c) & mask )) h3=$(( (h3 + d) & mask ))
    h4=$(( (h4 + e) & mask )) h5=$(( (h5 + f) & mask ))
    h6=$(( (h6 + g) & mask )) h7=$(( (h7 + h) & mask ))
  done
  printf -v DIGEST '%08x%08x%08x%08x%08x%08x%08x%08x' \
    "$h0" "$h1" "$h2" "$h3" "$h4" "$h5" "$h6" "$h7"
}

# add_check ID STATUS: append one check to CHECKS (a JSON list without brackets).
add_check() {
  CHECKS+="${CHECKS:+,}{\"id\":\"$1\",\"status\":\"$2\"}"
}

# add_diagnostic CODE MESSAGE: append one diagnostic, at most 8 in all. Both
# are fixed words or text the tool already restricted to plain characters.
add_diagnostic() {
  (( DIAGNOSTIC_COUNT < 8 )) || return 0
  DIAGNOSTIC_COUNT=$(( DIAGNOSTIC_COUNT + 1 ))
  DIAGNOSTICS+="${DIAGNOSTICS:+,}{\"code\":\"$1\",\"message\":\"$2\"}"
}

CHECKS=""
DIAGNOSTICS=""
DIAGNOSTIC_COUNT=0

# emit_result TEXT: print the result for the output TEXT, with CHECKS and
# DIAGNOSTICS, bound to the request that was read.
emit_result() {
  text_to_bytes "$1"
  sha256_bytes
  b64_encode
  printf '%s' "{\"protocol\":\"profile-tool.result/v1\",\"request_id\":\"$REQ_ID\",\"tool_id\":\"$REQ_TOOL\",\"runtime_sha256\":\"$REQ_RUNTIME\",\"input_revision\":\"$REQ_REVISION\",\"schema_sha256\":\"$REQ_SCHEMA\",\"checks\":[$CHECKS],\"outputs\":[{\"id\":\"$REQ_OUTPUT\",\"sha256\":\"$DIGEST\",\"bytes_base64\":\"$B64\"}],\"diagnostics\":[$DIAGNOSTICS]}"
}
