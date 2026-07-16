import SpaceInvadersGame from './components/SpaceInvadersGame';

export default function Home() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center bg-slate-950 text-white">
      <div className="relative z-10 flex flex-col items-center gap-8 p-4">
        <header className="text-center space-y-2">
          <h1 className="text-5xl font-extrabold tracking-tighter bg-gradient-to-r from-green-400 to-blue-500 bg-clip-text text-transparent uppercase italic">
            Space Invaders
          </h1>
          <p className="text-slate-400 font-mono text-sm uppercase tracking-widest">
            Defend the Galaxy from the Alien Invasion
          </p>
        </header>

        <div className="relative group border-4 border-slate-800 rounded-xl overflow-hidden shadow-2xl shadow-green-500/20 transition-shadow hover:shadow-green-500/40">
          <SpaceInvadersGame />
        </div>

        <footer className="text-center text-slate-500 font-mono text-xs space-y-1">
          <p>Controls: [←][→] Move | [SPACE] Shoot</p>
          <p>Quick Reset: Press [R]</p>
        </footer>
      </div>

      {/* Decorative background elements */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden -z-10">
        <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-green-500/10 blur-[120px] rounded-full" />
        <div className="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-blue-500/10 blur-[120px] rounded-full" />
      </div>
    </main>
  );
}
