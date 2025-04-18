class UrlRegistries:
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(UrlRegistries, cls).__new__(cls)
            cls._instance._gtnh_modpack = {}
            cls._instance._ulti_mc_launcher = None
            cls._instance._initialize_defaults()
        return cls._instance

    def _initialize_defaults(self):
        # Initialize the GTNH modpack dictionary
        self.add_to_gtnh_modpack(
            "2.6.0",
            "https://www.dropbox.com/scl/fi/rdn46wbc3a88xvpznh86o/GT_New_Horizons_2.6.0_Java_17-21.zip?rlkey=zu4xcrlz4xlzv5krgkh707hg7&e=1&dl=1"
        )
        self.add_to_gtnh_modpack(
            "2.6.1",
            "https://www.dropbox.com/scl/fi/g5dw3yp8g8jg17pb3vnhz/GT_New_Horizons_2.6.1_Java_17-21.zip?rlkey=zgaey4suyoytofeuoqnj85uw7&e=1&dl=1"
        )
        self.add_to_gtnh_modpack(
            "2.7.0",
            "https://www.dropbox.com/scl/fi/x98sqnvvkag3mn5x3p5rl/GT_New_Horizons_2.7.0_Java_17-21.zip?rlkey=a36ksdkidkizm3oa0gpivarst&e=1&dl=1"
        )
        self.add_to_gtnh_modpack(
            "2.7.1",
            "https://www.dropbox.com/scl/fi/0j06t07lxtgu8l6kuyo3a/GT_New_Horizons_2.7.1_Java_17-21.zip?rlkey=0drdoneib30mn9dciw46jh471&e=1&dl=1"
        )
        self.add_to_gtnh_modpack(
            "2.7.2",
            "https://www.dropbox.com/scl/fi/5otmq7bgiiy2teonqzbl5/GT_New_Horizons_2.7.2_Java_17-21.zip?rlkey=lyfr64aecc2e1pu1us2j3xd58&dl=1"
        )

        # Initialize the UltiMC launcher URL
        self._ulti_mc_launcher = "https://www.dropbox.com/scl/fi/d9d50x5dpuv9417yaqs9y/mmc-cracked-win32.zip?rlkey=kwsqzf72jh61x24bykvtnimqm&st=uknrms88&dl=1"

        # Initialize the JDK URL
        self._jdk_url = "https://www.dropbox.com/scl/fi/72085q4fpnzaixwefbkc4/jdk-21.0.3-9.rar?rlkey=kvbnb13rl10tluptcctj388lr&st=wx50g84z&dl=0"

    @property
    def gtnh_modpack_url(self):
        return self._gtnh_modpack

    @property
    def ulti_mc_launcher_url(self):
        return self._ulti_mc_launcher

    @ulti_mc_launcher_url.setter
    def ulti_mc_launcher_url(self, value):
        self._ulti_mc_launcher = value

    def add_to_gtnh_modpack(self, key, value):
        """Add a new entry to the GTNH modpack dictionary."""
        self._gtnh_modpack[key] = value

    def get_from_gtnh_modpack(self, key):
        """Retrieve a value from the GTNH modpack dictionary."""
        return self._gtnh_modpack.get(key)

    def get_gtnh_modpack(self):
        """Get the entire GTNH modpack dictionary."""
        return self._gtnh_modpack

    @property
    def jdk_url(self):
        return self._jdk_url

    @jdk_url.setter
    def jdk_url(self, value):
        self._jdk_url = value
