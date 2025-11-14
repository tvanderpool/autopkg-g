#!/bin/zsh

if [ -z "$ZSH_VERSION" ]; then
    exec zsh "$0" "$@"
    exit
fi

# sudo chown -R $(whoami): /cmdhistory

DCZSHRC="$(dirname "${(%):-%N}")/devcontainer.zshrc"

if ! grep -q "source $DCZSHRC" ~/.zshrc; then
    echo "source $DCZSHRC" >> ~/.zshrc
fi

# if ! grep -q "# ======Custom configuration======" ~/.zshrc; then
# cat <<EOF >> ~/.zshrc
# # ======Custom configuration======
# export HISTFILE='/cmdhistory/.zsh_history'
# export HISTSIZE=10000
# export SAVEHIST=10000
# export HISTFILESIZE=10000
# export HISTCONTROL=ignoredups:erasedups
# export HISTIGNORE='ls:cd:cd -:pwd:exit'
# [[ "$TERM_PROGRAM" == "vscode" ]] && . "\$(code --locate-shell-integration-path zsh)"
# EOF
# fi

# sed -i 's|^# \(COMPLETION_WAITING_DOTS="true"\)|\1|g' ~/.zshrc
# # enable vscode & rust plugins
# sed -i 's|plugins=(\(.*\))|plugins=(\1 vscode rust)|g' ~/.zshrc


# Alternative approach - source omz first
# source ~/.oh-my-zsh/oh-my-zsh.sh 2>/dev/null || true
source ~/.zshrc
omz update &>/dev/null || true
