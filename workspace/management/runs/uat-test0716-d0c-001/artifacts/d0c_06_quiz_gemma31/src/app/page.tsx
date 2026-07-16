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

type Phase = 'start' | 'quiz' | 'result';

export default function QuizPage() {
  const [phase, setPhase] = useState<Phase>('start');
  const [questionIndex, setQuestionIndex] = useState(0);
  const [score, setScore] = useState(0);

  const startQuiz = () => {
    setPhase('quiz');
    setQuestionIndex(0);
    setScore(0);
  };

  const handleAnswer = (index: number) => {
    if (index === QUIZ_DATA[questionIndex].correctAnswer) {
      setScore((prev) => prev + 1);
    }

    if (questionIndex < QUIZ_DATA.length - 1) {
      setQuestionIndex((prev) => prev + 1);
    } else {
      setPhase('result');
    }
  };

  const restartQuiz = () => {
    setPhase('start');
    setQuestionIndex(0);
    setScore(0);
  };

  const stateSnapshot = JSON.stringify({ phase, questionIndex, score });

  return (
    <div 
      className="min-h-screen bg-gray-100 flex items-center justify-center p-4"
      data-anvil-state={stateSnapshot}
    >
      <div className="bg-white p-8 rounded-2xl shadow-xl max-w-md w-full text-center">
        {phase === 'start' && (
          <div>
            <h1 className="text-3xl font-bold mb-6 text-gray-800">Knowledge Quiz</h1>
            <p className="text-gray-600 mb-8">Test your knowledge with 3 quick questions!</p>
            <button
              onClick={startQuiz}
              data-anvil-action="primary"
              className="bg-blue-600 hover:bg-blue-700 text-white font-bold py-3 px-8 rounded-full transition-colors duration-200 ease-in-out transform hover:scale-105"
            >
              Start Quiz
            </button>
          </div>
        )}

        {phase === 'quiz' && (
          <div>
            <div className="mb-4 text-sm font-medium text-blue-600">
              Question {questionIndex + 1} of {QUIZ_DATA.length}
            </div>
            <h2 className="text-2xl font-semibold mb-6 text-gray-800">
              {QUIZ_DATA[questionIndex].question}
            </h2>
            <div className="grid gap-4">
              {QUIZ_DATA[questionIndex].options.map((option, index) => (
                <button
                  key={index}
                  onClick={() => handleAnswer(index)}
                  data-anvil-action="primary"
                  className="bg-gray-50 hover:bg-blue-50 border-2 border-gray-200 hover:border-blue-400 text-gray-700 py-3 px-4 rounded-xl transition-all duration-150 text-left font-medium"
                >
                  {option}
                </button>
              ))}
            </div>
          </div>
        )}

        {phase === 'result' && (
          <div>
            <h2 className="text-3xl font-bold mb-4 text-gray-800">Quiz Completed!</h2>
            <div className="text-6xl font-extrabold text-blue-600 mb-6">
              {score} / {QUIZ_DATA.length}
            </div>
            <p className="text-gray-600 mb-8">
              {score === QUIZ_DATA.length 
                ? "Perfect score! You're a genius!" 
                : score > 0 
                  ? "Good job! Keep practicing." 
                  : "Better luck next time!"}
            </p>
            <button
              onClick={restartQuiz}
              data-anvil-action="restart"
              className="bg-blue-600 hover:bg-blue-700 text-white font-bold py-3 px-8 rounded-full transition-colors duration-200 ease-in-out transform hover:scale-105"
            >
              Try Again
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
