# Development requirements

- Keep all characters for the displayed purpose visible together in the upper team area. The current SNS workflow uses four roles; historical workflows can use five. Include all five bundled characters. Do not hide waiting or completed characters behind scrolling, tabs, or merged roles.
- Keep request/decisions/activity on the left and document/posting preparation on the right. Work-pane tabs must preserve the team area and workbench height.
- Display actual account identity, role, state, attempt and artifact identities. Missing evidence stays unknown. A character animation is never proof of completion or publication.
- Preserve execution, approval, recovery and one-time submission checks. Unsupported destinations must not enable sending. UI changes do not authorize new inference, service calls, posting or spending.
- Keep code licensing separate from artwork licensing. Preserve CHARACTER_ASSETS_LICENSE.md when distributing bundled or modified characters.
- Use isolated test data. Never commit user databases, credentials, environment files, local histories or private evidence. Tests must not require the author's local directory or account.
- Run npm run check, npm run build, npm run test:boundary and the Windows native offline tests for changes that affect these paths. Real-service tests require separate explicit authorization.
