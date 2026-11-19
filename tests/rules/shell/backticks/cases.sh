now=`date`
#   ^^^^^^ no-backticks
echo "`date +%s`" done
#     ^^^^^^^^^^ no-backticks
esc=`printf "\\n"`
#   ^^^^^^^^^^^^^^ no-backticks
