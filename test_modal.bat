@echo off
REM Lancer la version de test d'Abcom avec la modal TOUJOURS visible
cd /d "%LOCALAPPDATA%\abcom"
echo Lancement de la version de TEST (modal toujours visible)
echo Si tu vois la modal "Créer un groupe - TEST", c'est que le problème est avec le button/condition
echo Sinon, c'est un problème avec l'affichage de la modal elle-même
echo.
abcom_test.exe ra
