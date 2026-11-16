# shellcheck shell=bash
#
# Stage `check`: read work/items.json (what stage `parse` committed) and
# propose out/report.json, `{"items":<count>,"total":<sum of quantities>}`.
#
# Two checks: `unique_names` (no name twice) and `quantities` (each quantity is
# zero or more). A failed check is reported with exit 0, so the model can be
# told and submit again; the host adopts the report only when both pass.

# shellcheck source-path=SCRIPTDIR source=lib.sh
source "${BASH_SOURCE[0]%/*}/lib.sh"

read_request || exit 3
b64_decode "$REQ_INPUT" || exit 4

names=()
count=0
total=0
unique=passed
quantities=passed
shape=ok

if bytes_to_text; then
  item_pattern='^\{"name":"([^"]*)","quantity":(-?[0-9]+)\}(,?)(.*)$'
  rest=${TEXT#'{"items":['}
  if [ "$rest" = "$TEXT" ]; then shape=bad; fi
  while [ "$shape" = ok ] && [[ $rest =~ $item_pattern ]]; do
    name=${BASH_REMATCH[1]}
    quantity=${BASH_REMATCH[2]}
    more=${BASH_REMATCH[3]}
    rest=${BASH_REMATCH[4]}
    for seen in "${names[@]}"; do
      if [ "$seen" = "$name" ]; then
        unique=failed
        add_diagnostic duplicate_name "the name $name appears more than once"
      fi
    done
    names[count]=$name
    if (( quantity < 0 )); then
      quantities=failed
      add_diagnostic negative_quantity "the quantity of $name is below zero"
    fi
    count=$(( count + 1 ))
    total=$(( total + quantity ))
    [ -n "$more" ] || break
  done
  if [ "$rest" != "]}"$'\n' ] || [ "$count" -eq 0 ]; then shape=bad; fi
else
  shape=bad
fi

if [ "$shape" != ok ]; then
  unique=failed
  quantities=failed
  add_diagnostic shape "work/items.json is not the list stage parse writes"
fi

add_check unique_names "$unique"
add_check quantities "$quantities"
emit_result "{\"items\":$count,\"total\":$total}"$'\n'
