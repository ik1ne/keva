using Microsoft.Win32;
using WixToolset.Dtf.WindowsInstaller;

namespace Keva.Installer
{
    public class Actions
    {
        /// <summary>
        /// Removes Keva startup registry entries for all users.
        /// Enumerates HKEY_USERS to access each user's HKCU hive.
        /// </summary>
        [CustomAction]
        public static ActionResult RemoveStartupRegistry(Session session)
        {
            session.Log("RemoveStartupRegistry: Starting");

            string[] valuesToRemove = { "Keva", "Keva (Debug)" };

            try
            {
                using (var hkeyUsers = Registry.Users)
                {
                    foreach (var sid in hkeyUsers.GetSubKeyNames())
                    {
                        // Skip system accounts and _Classes keys
                        if (sid.StartsWith(".") || sid.EndsWith("_Classes"))
                            continue;

                        try
                        {
                            var runKeyPath = $@"{sid}\SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
                            using (var runKey = hkeyUsers.OpenSubKey(runKeyPath, writable: true))
                            {
                                if (runKey == null) continue;

                                foreach (var valueName in valuesToRemove)
                                {
                                    if (runKey.GetValue(valueName) != null)
                                    {
                                        runKey.DeleteValue(valueName, throwOnMissingValue: false);
                                        session.Log($"RemoveStartupRegistry: Removed '{valueName}' for SID {sid}");
                                    }
                                }
                            }
                        }
                        catch
                        {
                            // Skip inaccessible profiles
                        }
                    }
                }
            }
            catch (System.Exception ex)
            {
                session.Log($"RemoveStartupRegistry: Error - {ex.Message}");
            }

            session.Log("RemoveStartupRegistry: Complete");
            return ActionResult.Success;
        }
    }
}
