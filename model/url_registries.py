class Versions:
    _instance = None
    _dict = {}

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(Versions, cls).__new__(cls)
            cls._instance.add_to_dict("2.6.0",
                                      "https://downloads.gtnewhorizons.com/Multi_mc_downloads/GT_New_Horizons_2.6.0_Java_17-21.zip")
            cls._instance.add_to_dict("2.6.1",
                                      "https://downloads.gtnewhorizons.com/Multi_mc_downloads/GT_New_Horizons_2.6.1_Java_17-21.zip")
            cls._instance.add_to_dict("2.7.0",
                                      "https://downloads.gtnewhorizons.com/Multi_mc_downloads/GT_New_Horizons_2.7.0_Java_17-21.zip")
            cls._instance.add_to_dict("2.7.1",
                                      "https://downloads.gtnewhorizons.com/Multi_mc_downloads/GT_New_Horizons_2.7.1_Java_17-21.zip")
            cls._instance.add_to_dict("2.7.2",
                                      "https://downloads.gtnewhorizons.com/ServerPacks/GT_New_Horizons_2.7.2_Server_Java_17-21.zip")
        return cls._instance

    def add_to_dict(self, key, value):
        self._dict[key] = value

    def get_from_dict(self, key):
        return self._dict.get(key)

    def get_dict(self):
        return self._dict
