"use client";

import React, { useState } from 'react';

type Question = {
  question: string;
  options: string[];
  correctAnswer: number;
};

const QUIZ_DATA: Question[] = [
  {
    question: "What is the capital of France?",
    options: ["London", "Berlin", "Paris", "Madrid"],
    correctAnswer: 2,
  },
  {
    question: "Which planet is known as the Red Planet?",
    options: ["Venus", "Mars", "Jupiter", "Saturn"],
    correctAnswer: 1,
  },
  {
    question: "Who wrote 'Romeo and Juliet'?",
    options: ["Charles Dickens", "Mark Twain", "William Shakespeare", "Jane Austen"],
    correctAnswer: 2,
  },
];

type GameState = 'start' | 'playing' | 'finished';

export default function QuizPage() {
  const [gameState, setGameState] = useState<GameState>('start');
  const [currentQuestionIndex, setCurrentQuestionIndex] = useState(0);
  const [score, setScore] = useState(0);

  const handleStart = () => {
    setGameState('playing');
    setCurrentQuestionIndex(0);
    setScore(0);
  };

  const handleAnswer = (index: number) => {
    if (index === QUIZ_DATA[currentQuestionIndex].correctAnswer) {
      setScore((prev) => prev + 1);
    }

    const nextIndex = currentQuestionIndex + 1;
    if (nextIndex < QUIZ_DATA.length) {
      setCurrentQuestionIndex(nextIndex);
    } else {
      setGameState('finished');
    }
  };

  const handleRestart = () => {
    setGameState('start');
    setCurrentQuestionIndex(0);
    setScore(0);
  };

  const stateSnapshot = JSON.stringify({
    gameState,
    currentQuestionIndex,
    score,
  });

  return (
    <div 
      className="min-h-screen bg-gray-100 flex items-center justify-center p-4"
      data-anvil-state={stateSnapshot}
    >
      <div className="max-w-md w-full bg-white rounded-2xl shadow-xl p-8 text-center">
        {gameState === 'start' && (
          <div>
            <h1 className="text-3xl font-bold mb-6 text-gray-800">Welcome to the Quiz!</h1>
            <p className="text-gray-600 mb-8">Test your knowledge with 3 quick questions.</p>
            <button
              onClick={handleStart}
              data-anvil-action="primary"
              className="bg-blue-600 hover:bg-blue-700 text-white font-bold py-3 px-8 rounded-full transition-colors duration-200 w-full text-lg shadow-md"
            >
              Start Quiz
            </button>
          </div>
        )}

        {gameState === 'playing' && (
          <div>
            <div className="mb-4 flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">
                Question {currentQuestionIndex + 1} of {QUIZ_DATA.length}
              </span>
              <span className="text-sm font-medium text-blue-600">
                Score: {score}
              </span>
            </div>
            <h2 className="text-xl font-semibold mb-6 text-gray-800">
              {QUIZ_DATA[currentQuestionIndex].question}
            </h2>
            <div className="space-y-3">
              {QUIZ_DATA[currentQuestionIndex].options.map((option, index) => (
                <button
                  key={index}
                  onClick={() => handleAnswer(index)}
                  data-anvil-action="primary"
                  className="w-full text-left p-4 rounded-lg border-2 border-gray-200 hover:border-blue-500 hover:bg-blue-50 transition-all duration-150 text-gray-700 font-medium"
                >
                  {option}
                </button>
              ))}
            </div>
          </div>
        )}

        {gameState === 'finished' && (
          <div>
            <h2 className="text-3xl font-bold mb-4 text-gray-800">Quiz Completed!</h2>
            <div className="text-6xl font-extrabold text-blue-600 mb-6">
              {score} / {QUIZ_DATA.length}
            </div>
            <p className="text-gray-600 mb-8">
              {score === QUIZ_DATA.length ? "Perfect! You're a genius!" : 
               score >= 1 ? "Good job!" : "Better luck next time!"}
            </p>
            <button
              onClick={handleRestart}
              data-anvil-action="restart"
              className="bg-gray-800 hover:bg-gray-900 text-white font-bold py-3 px-8 rounded-full transition-colors duration-200 w-full text-lg shadow-md"
            >
              Try Again
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
