used=1
unused=1
^^^^^^ unused-variable
echo "$used" "$status" "${output:-}" "${missing[@]}"
#              ^^^^^^ unassigned-variable
#                                       ^^^^^^^ unassigned-variable
local_var() {
  local kept=1 dropped=2 bare
#              ^^^^^^^ unused-variable
  local -a list=(a b)
  echo "$kept" "${list[0]}"
}
read -r line rest
echo "$rest"
for i in 1 2; do echo "$i"; done
for j in 1 2; do :; done
#   ^ unused-variable
count=0
(( count += 1 ))
total=$((count * 2))
echo "$total"
export EXPORTED=1
PREFIXED=1 cmd
IFS=,
printf -v formatted '%s' x
echo "$formatted"
arr[0]=x
echo "${arr[0]}" "${#arr[@]}" "$#" "$@" "$1" "$_"
cat <<EOF
$heredoc_read ${FROM_ENV}
#^^^^^^^^^^^^ unassigned-variable
EOF
echo '$single' "\$escaped"
while getopts "ab:" opt; do
  case $opt in a) flag=1 ;; esac
done
echo "$flag"
