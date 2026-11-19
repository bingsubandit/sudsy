a="x "\""$v"\"" y"
# ^^^^^^^^^^^^^^^^ quote-in-quotes
d=($(cmd "$a"))
#  ^^^^^^^^^^^ split-command-output
l+=($dl)
#   ^^^ quote-array-element
x=($dl "q" "$m")
#  ^^^ quote-array-element
flunk "failed: $@"
#              ^^ array-in-string
echo "a ${s[@]} b"
#       ^^^^^^^ array-in-string
echo "$@" "${s[@]}"
PGHOST= psql
^^^^^^^^^^^^ assignment-space
IFS= read -r line
PGHOST=db psql
unset words[0]
^^^^^^^^^^^^^^ unset-glob
unset "words[0]"
[[ $a == $b ]]
^^^^^^^^^^^^^^ unquoted-pattern
[[ $a == "$b" ]]
x=$(echo $y)
# ^^^^^^^^^^ echo-in-substitution
#        ^^ quote-variable
x=$(echo "a" | tr a b)
[[ $(ps | grep x) ]]
^^^^^^^^^^^^^^^^^^^^ test-grep-output
for l in $(cat file); do :; done
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ iterate-file-output
for l in "$@"; do :; done
eval "f ${args[@]}"
#       ^^^^^^^^^^ array-in-string
#       ^^^^^^^^^^ eval-array
A=1 echo "$A"
#         ^^ assignment-not-seen
A=1 cmd "$B"
A=1 cmd "$A_"
echo "$FUNCNAME" "${FUNCNAME[0]}"
#     ^^^^^^^^^ array-without-index
