zstyle ':omz:update' mode disabled
COMPLETION_WAITING_DOTS="true"
# export HISTFILE='/cmdhistory/.zsh_history'
export HISTSIZE=10000
export SAVEHIST=10000
export HISTFILESIZE=10000
export HISTCONTROL=ignoredups:erasedups
export HISTIGNORE='ls:cd:cd -:pwd:exit'
[[ "" == "vscode" ]] && . "$(code --locate-shell-integration-path zsh)"


#find ext
EXT=$(find ~/.vscode-server/extensions/vadimcn.vscode-lldb-* -maxdepth 0 -type d | head -n 1)
if ! [ -z "$EXT" ]; then
    if [ ! -f $EXT/adapter/codelldb-launch ] && [ -f $EXT/adapter/codelldb ]; then
        ln -sf $EXT/adapter/codelldb $EXT/adapter/codelldb-launch
    fi
fi
