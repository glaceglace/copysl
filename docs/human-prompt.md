1. 
This project is a GUI clipboard history. Here is what I want: 
In folder docs/ create a demand doc named demands.md. This demands.md shall fulfil my needs below
I will to create a clipboard history. use fltk-rs or egui or slint as framework. Use the most suitable framework that meet my demands.  Use the framework that you can generate the best quality of code. It shall support x11 and wayland. It shall be lightweight. It is better that the app doesn't need gpu but it is not mandatory. It shall not be based on electron or webview. Its ui shall looks like the clipboard history of windows: It shows a list of "cards" and each card shows the clipboard history. I can navigate card by keyboard arrows or mouse. When I click on one card or tap enter on one card that is highlighted, the clipboard windows shall be closed and paste the content to the current cursor place. Shortcut to call the clip board shall be Super+V on Linux. When shortcut is pressed, the clipboard window place shall show near the cursor.
Dont write any code at first. Create demand doc. Don't make decision for me, ask me any question if there is doubt or anything that is not clear.

2. 
Target: based on the demands doc create highly detailed design doc. Input: docs/demands.md Output: docs/detailed-design.md. Steps: Based on the demands.md content, split modules, recognize relations among modules and generate design doc. Don't imagine my thought. Ask me if you have any doubts or anything that is not clear

3. 
Target: Breakdown each module to  smallest executable tasks.
Input: Demand doc docs/demands.md, high detailed design doc : docs/detailed-design.md
output: Mission lists:
- doc/tasks/<module-name>.md (one file for each module)
- doc/tasks/progress.md (global progress)
Steps:
Based on demand doc and high detailed design doc. Generate smallest executable tasks for each module with <module-name>.md file.Use checklists to represent if sub task is done.
In progress.md, use checklists to represent if modules are done.

4. 
Target: Generate vibe coding prompts
Input: Demand doc: docs/demands.md, High detailed design doc: docs/detailed-design.md, task separations: docs/tasks/*
output: doc/prompt.md
Steps:
Read input files to understand what needs to be implemented.
generate doc/prompt.md as the start prompt for vibe coding.
Main agent: used to track global progress.
Main agent create child agents Each child agent is used to implement each module. No human interaction is needed during the implementation.
Code must contanis complete unit tests. Coverage shall be 100%. Use any existed rust tool to keep code quality.
While generating prompt.md ask me if you have any doubts or questions.