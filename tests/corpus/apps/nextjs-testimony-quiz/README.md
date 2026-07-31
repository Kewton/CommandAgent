# Next.js Quiz Application

A simple interactive quiz application built with Next.js and React.

## Features

- **Three hard-coded questions**: The quiz contains three multiple-choice questions.
- **Interactive answer buttons**: Click on an answer to select it and see if it's correct.
- **Real-time score tracking**: Your score updates after each answer selection.
- **Retry control**: A retry button allows you to restart the quiz at any time.

## How to Run

### Development Mode

```bash
npm run dev
```

This starts the development server on port 3011. Open [http://localhost:3011](http://localhost:3011) in your browser.

### Production Build

```bash
npm run build
```

This creates an optimized production build in the `.next` directory.

### Start Production Server

```bash
npm start
```

This starts the production server on port 3011.

## Project Structure

- `src/app/page.tsx` — The main quiz page component with all quiz logic.
- `src/app/layout.tsx` — The root layout component.
- `src/app/globals.css` — Global styles with Tailwind CSS directives.
- `package.json` — Project dependencies and scripts.

## Technologies Used

- Next.js 14
- React 18
- Tailwind CSS
- TypeScript
