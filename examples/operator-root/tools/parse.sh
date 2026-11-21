# shellcheck shell=bash
#
# Stage `parse`: read input/items.txt and propose work/items.json.
#
# Each non-empty line of the input is `name,quantity`. A name is letters,
# digits, spaces, `.`, `_`, or `-`; a quantity is a whole number (a sign is
# allowed here: stage `check` decides whether it is acceptable). The check
# `lines` fails when a line has another form or there is no item, and the
# process still exits 0, so the model can be told and submit again.

# shellcheck source-path=SCRIPTDIR source=lib.sh
source "${BASH_SOURCE[0]%/*}/lib.sh"

read_request || exit 3
b64_decode "$REQ_INPUT" || exit 4

status=passed
items=""
count=0
number=0
if ! bytes_to_text; then
  status=failed
  add_diagnostic not_text "the input holds a NUL byte"
  TEXT=""
fi

line_pattern='^([A-Za-z0-9_. -]{1,64}),(-?)([0-9]{1,9})$'
rest=$TEXT
while [ -n "$rest" ]; do
  line=${rest%%$'\n'*}
  if [[ $rest == *$'\n'* ]]; then rest=${rest#*$'\n'}; else rest=""; fi
  number=$(( number + 1 ))
  line=${line%$'\r'}
  [ -n "$line" ] || continue
  if [[ $line =~ $line_pattern ]]; then
    name=${BASH_REMATCH[1]}
    sign=${BASH_REMATCH[2]}
    quantity=$(( 10#${BASH_REMATCH[3]} ))
    if [ "$quantity" -eq 0 ]; then sign=""; fi
    items+="${items:+,}{\"name\":\"$name\",\"quantity\":$sign$quantity}"
    count=$(( count + 1 ))
  else
    status=failed
    add_diagnostic bad_line "line $number is not name,quantity"
  fi
done

if [ "$count" -eq 0 ]; then
  status=failed
  add_diagnostic no_items "the input has no item"
fi

add_check lines "$status"
emit_result "{\"items\":[$items]}"$'\n'
