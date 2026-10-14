"""Production OpenTUI + Rust backend in a dedicated controlling POSIX PTY.
Never attaches to the user's terminal or reads their configuration.
"""
import argparse, errno, fcntl, json, os, pathlib, pty, re, select, shutil, signal, struct, subprocess, sys, tempfile, termios, time
parser = argparse.ArgumentParser()
parser.add_argument('--root', required=True)
parser.add_argument('--case', choices=['success','failure','interrupt','backend-death','resize','ui-interrupt'], default='success')
options = parser.parse_args()
project = pathlib.Path(options.root).resolve()
backend = project / 'src-tauri/target/debug/norn-backend'
bun = shutil.which('bun'); git = shutil.which('git'); rustc = shutil.which('rustc')
assert bun and git and rustc and backend.is_file()
with tempfile.TemporaryDirectory(prefix='norn-owned-editor-pty-') as owned:
    root = pathlib.Path(owned).resolve(); repo = root / 'checkout with spaces'; repo.mkdir()
    config = root / 'config'; config.mkdir(); editors = root / 'editors'; editors.mkdir()
    source = repo / '--owned ü.txt'; source.write_text('owned line one\nsecond line\n')
    editor = editors / 'nvim'
    subprocess.run([rustc, '--edition=2021', str(project / 'ui/scripts/support/owned-editor.rs'), '-o', str(editor)], check=True, capture_output=True)
    env = {'PATH': os.pathsep.join([str(editors),str(pathlib.Path(git).parent),'/usr/bin','/bin']), 'TERM':'xterm-256color', 'LANG':'en_US.UTF-8', 'NORN_BACKEND_BIN':str(backend), 'NORN_BACKEND_REPO_PATH':str(repo), 'NORN_CONFIG_DIR':str(config),'NORN_DATA_DIR':str(root/'workspace'), 'NORN_REVIEW_DATA_DIR':str(root/'reviews'), 'NORN_OWNED_EDITOR_DIR':str(editors), 'VISUAL':str(editor), 'ZDOTDIR':str(root/'zsh'), 'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null','NORN_BROWSER_OPEN':'0'}
    pathlib.Path(env['ZDOTDIR']).mkdir()
    def run_git(*args): subprocess.run([git,'-C',str(repo),*args],env=env,check=True,capture_output=True)
    run_git('init','-q');run_git('remote','add','origin','https://github.com/owned/editor.git');run_git('config','user.name','Owned Editor');run_git('config','user.email','owned@example.invalid');run_git('add','--',source.name);run_git('-c','commit.gpgsign=false','commit','-qm','owned base')
    (config/'settings.json').write_text(json.dumps({'aiProvider':'claude','repos':[{'provider':'github','workspace':'owned','repo':'editor','localPath':str(repo)}],'defaultDiffView':'unified','theme':'dark'}))
    # Exercise local editor lifecycle without provider startup/credential probes.
    # Current-repo home/F8 bindings have native fake-transport input coverage.
    pid, master = pty.fork()
    if pid == 0:
        os.chdir(project/'ui'); os.execve(bun,[bun,'run','src/main.ts','--workspace'],env)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH',35,120,0,0))
    initial = termios.tcgetattr(master); output = bytearray(); exited = False
    def pump():
        if select.select([master],[],[],0.05)[0]:
            try: output.extend(os.read(master,65536))
            except OSError as error:
                if error.errno != errno.EIO: raise
    def wait(check, label):
        deadline = time.monotonic()+20
        while not check():
            pump()
            if time.monotonic()>deadline: raise AssertionError(label+'\n'+output.decode('utf-8','replace')[-12000:])
    try:
        wait(lambda: b'F8' in output,'Workspace did not render')
        if options.case == 'ui-interrupt':
            rows = subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,command=']).decode().splitlines()
            children = [int(parts[0]) for row in rows if len(parts := row.strip().split(None,2)) == 3 and int(parts[1]) == pid and parts[2] == str(backend)]
            assert len(children) == 1, 'Expected exactly our owned backend child'
            os.write(master,b'\x03')
            deadline=time.monotonic()+8
            while time.monotonic()<deadline:
                pump();got,status=os.waitpid(pid,os.WNOHANG)
                if got:
                    exited=True;assert os.waitstatus_to_exitcode(status)==0, 'UI Ctrl+C exit: '+str(os.waitstatus_to_exitcode(status));break
            assert exited,'UI Ctrl+C did not terminate'
            for child in children:
                try: os.kill(child,0)
                except ProcessLookupError: pass
                else: raise AssertionError('Owned backend survived UI Ctrl+C')
            mask=termios.ICANON|termios.ECHO|termios.ISIG
            assert initial[3]&mask==termios.tcgetattr(master)[3]&mask,'UI Ctrl+C did not restore terminal modes'
            print('PASS ui-interrupt: real UI Ctrl+C, backend reaping and terminal restoration')
            sys.exit(0)
        if options.case == 'resize':
            output.clear()
            fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',25,90,0,0))
            os.kill(pid,signal.SIGWINCH)
            wait(lambda: b'F8' in output,'Workspace did not redraw after actual PTY resize')
        output.clear(); os.write(master,b'\x1b[19~')
        wait(lambda: source.name.encode() in output,'F8 explorer inventory did not render')
        os.write(master,b'\r'); wait(lambda: b'owned line one' in output,'Selected source did not render')
        wait(lambda: b'o open' in output,'Workspace did not finish initial loading')
        output.clear(); os.write(master,b'O')
        wait(lambda: b'OWNED_EDITOR_READY' in output,'Actual editor did not acquire the terminal')
        assert (editors/'changed-modes.txt').is_file(), 'Editor did not change real terminal modes'
        assert (editors/'argv.txt').read_text() == '+1\n'+str(source), 'argv/source line changed'
        assert os.tcgetpgrp(master) != pid, 'Editor did not receive foreground ownership'
        if options.case == 'backend-death':
            wrapper = int((editors/'wrapper.pid').read_text())
            backend_pid = int(subprocess.check_output(['/bin/ps','-o','ppid=','-p',str(wrapper)]).strip())
            backend_command = subprocess.check_output(['/bin/ps','-o','command=','-p',str(backend_pid)]).decode().strip()
            backend_parent = int(subprocess.check_output(['/bin/ps','-o','ppid=','-p',str(backend_pid)]).strip())
            assert backend_command == str(backend) and backend_parent == pid, 'Death test does not own this backend'
            os.kill(backend_pid, signal.SIGKILL)
        else:
            os.write(master, b'\x03' if options.case == 'interrupt' else (b'fail\n' if options.case == 'failure' else b'finish\n'))
            if options.case != 'interrupt': wait(lambda: (editors/'input.txt').exists(),'Editor did not read real terminal input')
            wait(lambda: os.tcgetpgrp(master)==pid,'UI foreground was not restored')
            if options.case != 'interrupt': assert (editors/'input.txt').read_text().strip()==('fail' if options.case=='failure' else 'finish')
            output.clear();os.write(master,b'r');wait(lambda: b'FILES' in output,'UI did not resume after editor exit')
        if options.case != 'backend-death': os.write(master,b'q')
        deadline=time.monotonic()+8
        while time.monotonic()<deadline:
            pump(); got,status=os.waitpid(pid,os.WNOHANG)
            if got: exited=True;assert os.waitstatus_to_exitcode(status)==(137 if options.case=='backend-death' else 0);break
        assert exited,'UI did not quit cleanly'
        editor_pid = int((editors/'editor.pid').read_text())
        def editor_gone():
            try: os.kill(editor_pid,0); return False
            except ProcessLookupError: return True
        wait(editor_gone,'Owned editor survived workspace exit')
        restored=termios.tcgetattr(master)
        mask=termios.ICANON|termios.ECHO|termios.ISIG
        assert initial[3]&mask==restored[3]&mask, 'Terminal modes were not restored'
        print('PASS '+options.case+': real production UI/editor input, literal argv, foreground handoff, UI resume and terminal restoration')
    finally:
        if not exited:
            os.kill(pid,signal.SIGTERM)
            for _ in range(100):
                got,_=os.waitpid(pid,os.WNOHANG)
                if got: exited=True;break
                pump()
            if not exited: os.kill(pid,signal.SIGKILL);os.waitpid(pid,0)
        os.close(master)
