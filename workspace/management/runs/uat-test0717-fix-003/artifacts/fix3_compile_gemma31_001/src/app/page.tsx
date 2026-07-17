"use client";

import SpaceInvaders from "./components/SpaceInvaders";

export default function Page() {
  return (
    <div className="min-h-screen bg-black flex flex-col items-center justify-center">
      <h1 className="text-4xl font-bold text-green-400 mb-4">SPACE INVADERS</h1>
      <SpaceInvaders />
    </div>
  );
}
