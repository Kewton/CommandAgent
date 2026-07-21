"use client";

import { useState } from "react";

type Question = {
  question: string;
  options: string[];
  correctIndex: number;
};

const questions: Question[] = [
  {
    question: "日本の首都はどこですか？",
    options: ["大阪", "東京", "京都", "名古屋"],
    correctIndex: 1,
  },
  {
    question: "2 + 2 はいくつですか？",
    options: ["3", "5", "4", "6"],
    correctIndex: 2,
  },
  {
    question: "太陽系で最も大きい惑星は？",
    options: ["土星", "火星", "木星", "地球"],
    correctIndex: 2,
  },
];

export default function QuizPage() {
  const [currentQuestion, setCurrentQuestion] = useState(0);
  const [selectedAnswer, setSelectedAnswer] = useState<number | null>(null);
  const [score, setScore] = useState(0);
  const [showResult, setShowResult] = useState(false);
  const [answers, setAnswers] = useState<(number | null)[]>([]);

  const handleAnswer = (answerIndex: number) => {
    setSelectedAnswer(answerIndex);
    const isCorrect = answerIndex === questions[currentQuestion].correctIndex;
    if (isCorrect) {
      setScore((prev) => prev + 1);
    }
    setAnswers((prev) => [...prev, answerIndex]);
  };

  const handleNext = () => {
    if (currentQuestion + 1 < questions.length) {
      setCurrentQuestion((prev) => prev + 1);
      setSelectedAnswer(null);
    } else {
      setShowResult(true);
    }
  };

  const handleRestart = () => {
    setCurrentQuestion(0);
    setSelectedAnswer(null);
    setScore(0);
    setShowResult(false);
    setAnswers([]);
  };

  const state = {
    currentQuestion: showResult ? questions.length : currentQuestion + 1,
    totalQuestions: questions.length,
    score: score,
    showResult: showResult,
  };

  if (showResult) {
    return (
      <div
        className="min-h-screen bg-gradient-to-br from-blue-50 via-indigo-50 to-purple-50 flex items-center justify-center p-4"
        data-anvil-state={JSON.stringify(state)}
      >
        <div className="bg-white rounded-2xl shadow-xl p-8 max-w-md w-full text-center">
          <div className="mb-6">
            <div className="text-6xl mb-4">🎉</div>
            <h1 className="text-3xl font-bold text-gray-800 mb-2">
              クイズ完了！
            </h1>
            <p className="text-gray-500">あなたのスコア</p>
          </div>
          <div className="text-6xl font-bold text-indigo-600 mb-2">
            {score} / {questions.length}
          </div>
          <p className="text-gray-600 mb-8">
            {score === questions.length
              ? "素晴らしい！完璧なスコアです！"
              : score >= 2
                ? "よくできました！"
                : "もう一度挑戦してみましょう！"}
          </p>
          <button
            onClick={handleRestart}
            className="px-8 py-3 bg-indigo-600 text-white rounded-xl font-semibold text-lg hover:bg-indigo-700 transition-colors shadow-md"
            data-anvil-action="restart"
          >
            もう一度プレイ
          </button>
        </div>
      </div>
    );
  }

  const question = questions[currentQuestion];
  const isAnswered = selectedAnswer !== null;
  const isCorrect = selectedAnswer === question.correctIndex;

  return (
    <div
      className="min-h-screen bg-gradient-to-br from-blue-50 via-indigo-50 to-purple-50 flex items-center justify-center p-4"
      data-anvil-state={JSON.stringify(state)}
    >
      <div className="bg-white rounded-2xl shadow-xl p-8 max-w-lg w-full">
        {/* Progress bar */}
        <div className="mb-6">
          <div className="flex justify-between items-center mb-2">
            <span className="text-sm font-medium text-gray-500">
              問題 {currentQuestion + 1} / {questions.length}
            </span>
            <span className="text-sm font-medium text-indigo-600">
              スコア: {score}
            </span>
          </div>
          <div className="w-full bg-gray-200 rounded-full h-2">
            <div
              className="bg-indigo-600 h-2 rounded-full transition-all duration-300"
              style={{
                width: `${((currentQuestion + 1) / questions.length) * 100}%`,
              }}
            />
          </div>
        </div>

        {/* Question */}
        <h2 className="text-2xl font-bold text-gray-800 mb-6 text-center">
          {question.question}
        </h2>

        {/* Options */}
        <div className="space-y-3">
          {question.options.map((option, index) => {
            let buttonClass =
              "w-full p-4 rounded-xl border-2 text-left font-medium transition-all duration-200 ";
            if (isAnswered) {
              if (index === question.correctIndex) {
                buttonClass +=
                  "border-green-500 bg-green-50 text-green-700 ";
              } else if (index === selectedAnswer) {
                buttonClass +=
                  "border-red-500 bg-red-50 text-red-700 ";
              } else {
                buttonClass += "border-gray-200 text-gray-400 ";
              }
            } else {
              buttonClass +=
                "border-gray-200 hover:border-indigo-400 hover:bg-indigo-50 text-gray-700 ";
            }
            return (
              <button
                key={index}
                onClick={() => handleAnswer(index)}
                disabled={isAnswered}
                className={buttonClass}
                data-anvil-action="primary"
              >
                <span className="inline-block w-8 h-8 bg-indigo-100 text-indigo-700 rounded-lg text-center leading-8 mr-3 font-bold text-sm">
                  {String.fromCharCode(65 + index)}
                </span>
                {option}
              </button>
            );
          })}
        </div>

        {/* Next button */}
        {isAnswered && (
          <div className="mt-6 text-center">
            <button
              onClick={handleNext}
              className="px-8 py-3 bg-indigo-600 text-white rounded-xl font-semibold text-lg hover:bg-indigo-700 transition-colors shadow-md"
              data-anvil-action="primary"
            >
              {currentQuestion + 1 < questions.length
                ? "次の問題 →"
                : "結果を見る →"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
