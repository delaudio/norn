// Dedicated pseudoconsole for production UI tests. No handle from the user's console.
using System;
using System.IO;
using System.Text;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
public sealed class OwnedConPty : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Coord { public short X, Y; }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Startup {
        public uint cb; public string reserved, desktop, title;
        public uint x,y,xsize,ysize,xchars,ychars,fill,flags;
        public ushort show,reserved2; public IntPtr reservedPtr,input,output,error;
    }
    [StructLayout(LayoutKind.Sequential)] struct StartupEx { public Startup info; public IntPtr attributes; }
    [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process,thread; public uint pid,tid; }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Entry {
        public uint size,usage,pid; public UIntPtr heap; public uint module,threads,parent;
        public int priority; public uint flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=260)] public string exe;
    }
    [DllImport("kernel32",SetLastError=true)] static extern bool CreatePipe(out IntPtr read,out IntPtr write,IntPtr security,uint size);
    [DllImport("kernel32")] static extern int CreatePseudoConsole(Coord size,IntPtr input,IntPtr output,uint flags,out IntPtr console);
    [DllImport("kernel32")] static extern void ClosePseudoConsole(IntPtr console);
    [DllImport("kernel32",SetLastError=true)] static extern bool InitializeProcThreadAttributeList(IntPtr list,int count,uint flags,ref IntPtr bytes);
    [DllImport("kernel32",SetLastError=true)] static extern bool UpdateProcThreadAttribute(IntPtr list,uint flags,IntPtr attribute,IntPtr value,IntPtr bytes,IntPtr previous,IntPtr returned);
    [DllImport("kernel32")] static extern void DeleteProcThreadAttributeList(IntPtr list);
    [DllImport("kernel32",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessW(string application,StringBuilder command,IntPtr processAttributes,IntPtr threadAttributes,bool inherit,uint flags,IntPtr environment,string cwd,ref StartupEx startup,out ProcessInfo process);
    [DllImport("kernel32",SetLastError=true)] static extern bool ReadFile(IntPtr file,byte[] buffer,uint length,out uint read,IntPtr overlap);
    [DllImport("kernel32",SetLastError=true)] static extern bool WriteFile(IntPtr file,byte[] buffer,uint length,out uint written,IntPtr overlap);
    [DllImport("kernel32",SetLastError=true)] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
    [DllImport("kernel32",SetLastError=true)] static extern bool TerminateProcess(IntPtr process,uint code);
    [DllImport("kernel32",SetLastError=true)] static extern IntPtr CreateToolhelp32Snapshot(uint flags,uint pid);
    [DllImport("kernel32",CharSet=CharSet.Unicode)] static extern bool Process32FirstW(IntPtr snapshot,ref Entry entry);
    [DllImport("kernel32",CharSet=CharSet.Unicode)] static extern bool Process32NextW(IntPtr snapshot,ref Entry entry);
    IntPtr console,input,output,process;
    readonly StringBuilder text = new StringBuilder();
    readonly object sync = new object();
    Thread reader;
    public uint Pid {get;private set;}
    public string Output {get {lock(sync) return text.ToString();}}
    static void Check(bool valid,string action) {if(!valid) throw new IOException(action+": "+Marshal.GetLastWin32Error());}
    public static uint Parent(uint pid) {
        IntPtr snapshot=CreateToolhelp32Snapshot(2,0); Check(snapshot!=new IntPtr(-1),"process snapshot");
        try {
            Entry entry=new Entry();entry.size=(uint)Marshal.SizeOf(typeof(Entry));
            bool present=Process32FirstW(snapshot,ref entry);
            while(present) {if(entry.pid==pid)return entry.parent;present=Process32NextW(snapshot,ref entry);}
            throw new IOException("Owned process is no longer present");
        } finally {CloseHandle(snapshot);}
    }
    // Caller verifies the owned backend's parent and executable first. Retain
    // its native handle and inject a known exit cause rather than depending on
    // .NET Process.Kill's platform-specific exit status/Process cache.
    public static uint TerminateOwned(Process owned,uint exitCode) {
        IntPtr handle=owned.Handle;
        Check(TerminateProcess(handle,exitCode),"owned backend termination");
        Check(owned.WaitForExit(5000),"owned backend wait");
        uint actual;Check(GetExitCodeProcess(handle,out actual),"owned backend native exit");
        Check(actual==exitCode,"owned backend exit cause");
        return actual;
    }
    public OwnedConPty(string executable,string arguments,string cwd,string environmentBlock) {
        IntPtr pseudoInput=IntPtr.Zero,pseudoOutput=IntPtr.Zero,attributes=IntPtr.Zero,environment=IntPtr.Zero;
        bool attributesReady=false;
        try {
            Check(CreatePipe(out pseudoInput,out input,IntPtr.Zero,0),"input pipe");
            Check(CreatePipe(out output,out pseudoOutput,IntPtr.Zero,0),"output pipe");
            int result=CreatePseudoConsole(new Coord{X=120,Y=35},pseudoInput,pseudoOutput,0,out console);
            if(result!=0)throw new IOException("CreatePseudoConsole HRESULT "+result);
            IntPtr bytes=IntPtr.Zero;InitializeProcThreadAttributeList(IntPtr.Zero,1,0,ref bytes);
            attributes=Marshal.AllocHGlobal(bytes);Check(InitializeProcThreadAttributeList(attributes,1,0,ref bytes),"attribute list");attributesReady=true;
            Check(UpdateProcThreadAttribute(attributes,0,new IntPtr(0x20016),console,new IntPtr(IntPtr.Size),IntPtr.Zero,IntPtr.Zero),"pseudoconsole attribute");
            StartupEx startup=new StartupEx();startup.info.cb=(uint)Marshal.SizeOf(typeof(StartupEx));startup.attributes=attributes;
            environment=Marshal.StringToHGlobalUni(environmentBlock);
            ProcessInfo child;
            Check(CreateProcessW(executable,new StringBuilder("\""+executable+"\" "+arguments),IntPtr.Zero,IntPtr.Zero,false,0x00080000|0x00000400,environment,cwd,ref startup,out child),"owned UI launch");
            process=child.process;Pid=child.pid;CloseHandle(child.thread);
            // Retain the creation handles until the client is attached, as
            // required by the documented pseudoconsole-session lifecycle.
            CloseHandle(pseudoInput);pseudoInput=IntPtr.Zero;CloseHandle(pseudoOutput);pseudoOutput=IntPtr.Zero;
            reader=new Thread(ReadOutput){IsBackground=true};reader.Start();
        } catch {Dispose();throw;}
        finally {
            if(pseudoInput!=IntPtr.Zero)CloseHandle(pseudoInput);if(pseudoOutput!=IntPtr.Zero)CloseHandle(pseudoOutput);
            if(attributesReady)DeleteProcThreadAttributeList(attributes);if(attributes!=IntPtr.Zero)Marshal.FreeHGlobal(attributes);
            if(environment!=IntPtr.Zero)Marshal.FreeHGlobal(environment);
        }
    }
    void ReadOutput() {
        var decoder=Encoding.UTF8.GetDecoder();byte[] buffer=new byte[16384];char[] chars=new char[16384];uint count;
        while(ReadFile(output,buffer,(uint)buffer.Length,out count,IntPtr.Zero)&&count>0) {
            int length=decoder.GetChars(buffer,0,(int)count,chars,0);
            lock(sync) {text.Append(chars,0,length);if(text.Length>4000000)text.Remove(0,text.Length-2000000);}
        }
    }
    public void Send(string value) {byte[] bytes=Encoding.UTF8.GetBytes(value);uint written;Check(WriteFile(input,bytes,(uint)bytes.Length,out written,IntPtr.Zero)&&written==bytes.Length,"owned terminal input");}
    public int ExitCode {get {uint code;Check(GetExitCodeProcess(process,out code),"owned exit code");return code==259?-1:unchecked((int)code);}}
    public void Dispose() {
        if(process!=IntPtr.Zero) {uint code;if(GetExitCodeProcess(process,out code)&&code==259)TerminateProcess(process,125);CloseHandle(process);process=IntPtr.Zero;}
        // Drain output concurrently while closing the pseudoconsole, per native API.
        if(console!=IntPtr.Zero){ClosePseudoConsole(console);console=IntPtr.Zero;}
        if(input!=IntPtr.Zero){CloseHandle(input);input=IntPtr.Zero;}
        if(output!=IntPtr.Zero){CloseHandle(output);output=IntPtr.Zero;}
        if(reader!=null)reader.Join(2000);
    }
}
