let s:cpo_save=&cpo
set cpo&vim
inoremap <silent> <Plug>CocRefresh =coc#_complete()
inoremap <C-Right> <Right>
inoremap <C-Left> <Left>
inoremap <C-Down> <Down>
inoremap <C-Up> <Up>
inoremap <M-R> :bn
inoremap <M-N> :bp
inoremap <M-r> mz:m-2`za
inoremap <M-n> mz:m+`za
imap <F2> <Plug>(coc-rename)
imap <silent> <expr> <C-Space> coc#refresh()
imap <expr> <S-Tab> pumvisible() ? "\" : "\"
noremap  
noremap  "+y
noremap  <Down>
noremap  <Up>
noremap  <Left>
noremap  <Right>
noremap  "+p
omap <silent> % <Plug>(MatchitOperationForward)
xmap <silent> % <Plug>(MatchitVisualForward)
nmap <silent> % <Plug>(MatchitNormalForward)
nmap <silent> ,slr :DBListVar
nmap <silent> ,sap :'<,'>DBVarRangeAssign
nmap <silent> ,sal :.,.DBVarRangeAssign
nmap <silent> ,sas :1,$DBVarRangeAssign
nmap ,so <Plug>DBOrientationToggle
nmap ,sh <Plug>DBHistory
xmap <silent> ,stcl :exec "DBListColumn '".DB_getVisualBlock()."'"
nmap ,stcl <Plug>DBListColumn
nmap ,slv <Plug>DBListView
nmap ,slp <Plug>DBListProcedure
nmap ,slt <Plug>DBListTable
xmap <silent> ,slc :exec "DBListColumn '".DB_getVisualBlock()."'"
nmap ,slc <Plug>DBListColumn
nmap ,sbp <Plug>DBPromptForBufferParameters
nmap ,sdpa <Plug>DBDescribeProcedureAskName
xmap <silent> ,sdp :exec "DBDescribeProcedure '".DB_getVisualBlock()."'"
nmap ,sdp <Plug>DBDescribeProcedure
nmap ,sdta <Plug>DBDescribeTableAskName
xmap <silent> ,sdt :exec "DBDescribeTable '".DB_getVisualBlock()."'"
nmap ,sdt <Plug>DBDescribeTable
xmap <silent> ,sT :exec "DBSelectFromTableTopX '".DB_getVisualBlock()."'"
nmap ,sT <Plug>DBSelectFromTopXTable
nmap ,sta <Plug>DBSelectFromTableAskName
nmap ,stw <Plug>DBSelectFromTableWithWhere
xmap <silent> ,st :exec "DBSelectFromTable '".DB_getVisualBlock()."'"
nmap ,st <Plug>DBSelectFromTable
nmap <silent> ,sep :'<,'>DBExecRangeSQL
nmap <silent> ,sel :.,.DBExecRangeSQL
nmap <silent> ,sea :1,$DBExecRangeSQL
nmap ,sq <Plug>DBExecSQL
nmap ,sE <Plug>DBExecSQLUnderTopXCursor
xmap ,sE <Plug>DBExecVisualTopXSQL
noremap ,f <Plug>(coc-fix-current)
noremap ,se :set spelllang=en_us
noremap ,sd :set spelllang=de_20
noremap ,s? z=
noremap ,sa zg
noremap <silent> ,ss :call ToggleSpellChecking()
nnoremap ,o :BufExplorer
noremap ,m mmHmt:%s///ge'tzt'm
noremap 0 ^
noremap B V
noremap E E
noremap H S
noremap I B
noremap J N
vnoremap <silent> L :call VisualSelection()
nnoremap L R
onoremap L R
noremap U I
noremap V 
omap <silent> [% <Plug>(MatchitOperationMultiBackward)
xmap <silent> [% <Plug>(MatchitVisualMultiBackward)
nmap <silent> [% <Plug>(MatchitNormalMultiBackward)
noremap \il ooimpl  {}<Up>$<Left>i
noremap \fn ofn () {}<Up>$3<Left>i
noremap \t a<></>3<Left>i
noremap \c :-1read $HOME/.config/nvim/snippets/co
noremap \rust :-1read $HOME/.config/nvim/snippets/rusto
noremap \html :-1read $HOME/.config/nvim/snippets/html3<Down>14<Right>a
omap <silent> ]% <Plug>(MatchitOperationMultiForward)
xmap <silent> ]% <Plug>(MatchitVisualMultiForward)
nmap <silent> ]% <Plug>(MatchitNormalMultiForward)
xmap a% <Plug>(MatchitVisualTextObject)
noremap b v
noremap e e
vmap gx <Plug>NetrwBrowseXVis
nmap gx <Plug>NetrwBrowseX
omap <silent> g% <Plug>(MatchitOperationBackward)
xmap <silent> g% <Plug>(MatchitVisualBackward)
nmap <silent> g% <Plug>(MatchitNormalBackward)
nmap <silent> gr <Plug>(coc-references)
nmap <silent> gi <Plug>(coc-implementation)
nmap <silent> gy <Plug>(coc-type-definition)
nmap <silent> gd <Plug>(coc-definition)
noremap h s
noremap i b
noremap j n
noremap l r
noremap n j
noremap r k
noremap s h
noremap t l
noremap u i
noremap v u
vnoremap <silent> <Plug>NetrwBrowseXVis :call netrw#BrowseXVis()
nnoremap <silent> <Plug>NetrwBrowseX :call netrw#BrowseX(expand((exists("g:netrw_gx")? g:netrw_gx : '<cfile>')),netrw#CheckIfRemote())
vmap <silent> <Plug>(MatchitVisualTextObject) <Plug>(MatchitVisualMultiBackward)o<Plug>(MatchitVisualMultiForward)
onoremap <silent> <Plug>(MatchitOperationMultiForward) :call matchit#MultiMatch("W",  "o")
onoremap <silent> <Plug>(MatchitOperationMultiBackward) :call matchit#MultiMatch("bW", "o")
vnoremap <silent> <Plug>(MatchitVisualMultiForward) :call matchit#MultiMatch("W",  "n")m'gv``
vnoremap <silent> <Plug>(MatchitVisualMultiBackward) :call matchit#MultiMatch("bW", "n")m'gv``
nnoremap <silent> <Plug>(MatchitNormalMultiForward) :call matchit#MultiMatch("W",  "n")
nnoremap <silent> <Plug>(MatchitNormalMultiBackward) :call matchit#MultiMatch("bW", "n")
onoremap <silent> <Plug>(MatchitOperationBackward) :call matchit#Match_wrapper('',0,'o')
onoremap <silent> <Plug>(MatchitOperationForward) :call matchit#Match_wrapper('',1,'o')
vnoremap <silent> <Plug>(MatchitVisualBackward) :call matchit#Match_wrapper('',0,'v')m'gv``
vnoremap <silent> <Plug>(MatchitVisualForward) :call matchit#Match_wrapper('',1,'v')m'gv``
nnoremap <silent> <Plug>(MatchitNormalBackward) :call matchit#Match_wrapper('',0,'n')
nnoremap <silent> <Plug>(MatchitNormalForward) :call matchit#Match_wrapper('',1,'n')
onoremap <silent> <Plug>(coc-classobj-a) :call coc#rpc#request('selectSymbolRange', [v:false, '', ['Interface', 'Struct', 'Class']])
onoremap <silent> <Plug>(coc-classobj-i) :call coc#rpc#request('selectSymbolRange', [v:true, '', ['Interface', 'Struct', 'Class']])
vnoremap <silent> <Plug>(coc-classobj-a) :call coc#rpc#request('selectSymbolRange', [v:false, visualmode(), ['Interface', 'Struct', 'Class']])
vnoremap <silent> <Plug>(coc-classobj-i) :call coc#rpc#request('selectSymbolRange', [v:true, visualmode(), ['Interface', 'Struct', 'Class']])
onoremap <silent> <Plug>(coc-funcobj-a) :call coc#rpc#request('selectSymbolRange', [v:false, '', ['Method', 'Function']])
onoremap <silent> <Plug>(coc-funcobj-i) :call coc#rpc#request('selectSymbolRange', [v:true, '', ['Method', 'Function']])
vnoremap <silent> <Plug>(coc-funcobj-a) :call coc#rpc#request('selectSymbolRange', [v:false, visualmode(), ['Method', 'Function']])
vnoremap <silent> <Plug>(coc-funcobj-i) :call coc#rpc#request('selectSymbolRange', [v:true, visualmode(), ['Method', 'Function']])
nnoremap <silent> <Plug>(coc-cursors-position) :call coc#rpc#request('cursorsSelect', [bufnr('%'), 'position', 'n'])
nnoremap <silent> <Plug>(coc-cursors-word) :call coc#rpc#request('cursorsSelect', [bufnr('%'), 'word', 'n'])
vnoremap <silent> <Plug>(coc-cursors-range) :call coc#rpc#request('cursorsSelect', [bufnr('%'), 'range', visualmode()])
nnoremap <silent> <Plug>(coc-refactor) :call       CocActionAsync('refactor')
nnoremap <silent> <Plug>(coc-command-repeat) :call       CocAction('repeatCommand')
nnoremap <silent> <Plug>(coc-float-jump) :call       coc#float#jump()
nnoremap <silent> <Plug>(coc-float-hide) :call       coc#float#close_all()
nnoremap <silent> <Plug>(coc-fix-current) :call       CocActionAsync('doQuickfix')
nnoremap <silent> <Plug>(coc-openlink) :call       CocActionAsync('openLink')
nnoremap <silent> <Plug>(coc-references-used) :call       CocActionAsync('jumpUsed')
nnoremap <silent> <Plug>(coc-references) :call       CocActionAsync('jumpReferences')
nnoremap <silent> <Plug>(coc-type-definition) :call       CocActionAsync('jumpTypeDefinition')
nnoremap <silent> <Plug>(coc-implementation) :call       CocActionAsync('jumpImplementation')
nnoremap <silent> <Plug>(coc-declaration) :call       CocActionAsync('jumpDeclaration')
nnoremap <silent> <Plug>(coc-definition) :call       CocActionAsync('jumpDefinition')
nnoremap <silent> <Plug>(coc-diagnostic-prev-error) :call       CocActionAsync('diagnosticPrevious', 'error')
nnoremap <silent> <Plug>(coc-diagnostic-next-error) :call       CocActionAsync('diagnosticNext',     'error')
nnoremap <silent> <Plug>(coc-diagnostic-prev) :call       CocActionAsync('diagnosticPrevious')
nnoremap <silent> <Plug>(coc-diagnostic-next) :call       CocActionAsync('diagnosticNext')
nnoremap <silent> <Plug>(coc-diagnostic-info) :call       CocActionAsync('diagnosticInfo')
nnoremap <silent> <Plug>(coc-format) :call       CocActionAsync('format')
nnoremap <silent> <Plug>(coc-rename) :call       CocActionAsync('rename')
nnoremap <Plug>(coc-codeaction-line) :call       CocActionAsync('codeAction',         'n')
nnoremap <Plug>(coc-codeaction) :call       CocActionAsync('codeAction',         '')
vnoremap <silent> <Plug>(coc-codeaction-selected) :call       CocActionAsync('codeAction',         visualmode())
vnoremap <silent> <Plug>(coc-format-selected) :call       CocActionAsync('formatSelected',     visualmode())
nnoremap <Plug>(coc-codelens-action) :call       CocActionAsync('codeLensAction')
nnoremap <Plug>(coc-range-select) :call       CocActionAsync('rangeSelect',     '', v:true)
vnoremap <silent> <Plug>(coc-range-select-backward) :call       CocActionAsync('rangeSelect',     visualmode(), v:false)
vnoremap <silent> <Plug>(coc-range-select) :call       CocActionAsync('rangeSelect',     visualmode(), v:true)
noremap <C-Right> <Right>
noremap <C-Left> <Left>
noremap <C-Down> <Down>
noremap <C-Up> <Up>
noremap <M-R> :bn
noremap <M-N> :bp
noremap <M-r> mz:m-2`z
noremap <M-n> mz:m+`z
noremap <PageDown> ]s
noremap <PageUp> [s
nmap <F2> <Plug>(coc-rename)
nmap <silent> <expr> <C-Space> coc#refresh()
inoremap  <Down>
inoremap  <Up>
inoremap  <Left>
inoremap  <Right>
inoremap  "+pa
noremap Ä 
noremap ä 	
noremap <silent> ß :noswapfile enew:setlocal buftype=nofile:setlocal bufhidden=hidefile scratch
cmap W w !sudo tee > /dev/null %
let &cpo=s:cpo_save
unlet s:cpo_save
set completeopt=menu
set expandtab
set helplang=en
set hidden
set lazyredraw
set matchtime=2
set mouse=a
set mousemodel=popup_setpos
set nrformats=bin,hex,octal
set path=.,/usr/include,,,**
set runtimepath=~/.config/nvim,~/.vim/plugged/vim-colorschemes/,~/.vim/plugged/coc.nvim/,~/.vim/plugged/bufexplorer/,~/.vim/plugged/vim-toml/,~/.vim/plugged/nginx.vim/,~/.vim/plugged/vim-better-whitespace/,~/.vim/plugged/vim-wasm/,~/.vim/plugged/vim-glsl/,~/.vim/plugged/vimtex/,~/.vim/plugged/coc-vimtex/,~/.vim/plugged/coc-json/,~/.vim/plugged/coc-clangd/,~/.vim/plugged/coc-rust-analyzer/,~/.vim/plugged/coc-yank/,~/.vim/plugged/coc-python/,~/.vim/plugged/coc-xml/,~/.vim/plugged/coc-java/,~/.vim/plugged/coc-html/,~/.vim/plugged/coc-omnisharp/,~/.vim/plugged/coc-yaml/,~/.vim/plugged/coc-tsserver/,~/.vim/plugged/coc-markdownlint/,~/.vim/plugged/dbext.vim/,/etc/xdg/nvim,~/.local/share/nvim/site,/usr/local/share/nvim/site,/usr/share/nvim/site,/usr/share/nvim/runtime,/usr/share/nvim/runtime/pack/dist/opt/matchit,/usr/share/nvim/site/after,/usr/local/share/nvim/site/after,~/.local/share/nvim/site/after,/etc/xdg/nvim/after,~/.config/nvim/after,~/.vim/plugged/vim-glsl/after,~/.vim/plugged/vimtex/after,/usr/share/vim/vimfiles
set scrolloff=5
set shiftwidth=4
set shortmess=filnxtToOFc
set showmatch
set smartcase
set spelllang=en_us
set spellsuggest=best,20
set statusline=%!UnifiedGensl()
set suffixes=.bak,~,.o,.h,.info,.swp,.obj,.lock
set tabstop=4
set timeoutlen=500
set undodir=~/.vim_runtime/temp_dirs/undodir
set undofile
set wildignore=*.o,*~,*.pyc,*/.git/*,*/.hg/*,*/.svn/*,*/.DS_Store,*/target/*
set wildmode=longest,list
set window=41
" vim: set ft=vim :
