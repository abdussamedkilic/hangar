import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import Modal from "./Modal";

function setup() {
  render(
    <Modal label="Deneme" onClose={vi.fn()}>
      <input aria-label="ilk" />
      <button>orta</button>
      <button>son</button>
    </Modal>,
  );
  return { first: screen.getByLabelText("ilk"), last: screen.getByRole("button", { name: "son" }) };
}

test("Tab on the last element goes to the first", () => {
  const { first, last } = setup();
  last.focus();
  fireEvent.keyDown(document, { key: "Tab" });
  expect(first).toHaveFocus();
});

test("Shift+Tab on the first element goes to the last", () => {
  const { first, last } = setup();
  first.focus();
  fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
  expect(last).toHaveFocus();
});

test("Tab in the middle is left to the browser", () => {
  setup();
  screen.getByRole("button", { name: "orta" }).focus();
  const notPrevented = fireEvent.keyDown(document, { key: "Tab" });
  expect(notPrevented).toBe(true);
});
